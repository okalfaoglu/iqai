# Observability & RCA for Automated Crypto Trading Systems

**Arşiv:** TFAI `TFAI-Q01` … `TFAI-Q14` için **tam teknik referans** (kaynak: Claude yanıtı, kullanıcı tarafından iletildi).  
**Anahtar şeması:** `docs/TRADE_FAILURE_AI_PROMPTS.md`  
**Dört model özeti + IQAI eşlemesi:** `docs/TRADE_FAILURE_AI_RESPONSES_SYNTHESIS.md`  
**Yatırım tavsiyesi içermez** — yalnızca mühendislik / gözlemlenebilirlik.

---

## TFAI-Q01 — Minimum Fields for Closed-Position RCA

A position record that can only tell you P&L is useless for debugging. You need to reconstruct *what the system believed, when it believed it, and what it did*.

**Identity & Lifecycle**

| Field | Type | Notes |
|---|---|---|
| `position_id` | UUID | Stable across partial fills |
| `strategy_id` | string | e.g. `momentum_v3` |
| `symbol` | string | `BTC/USDT` |
| `exchange` | string | `binance_spot` |
| `opened_at` | timestamp (μs) | Exchange-ack time, not local |
| `closed_at` | timestamp (μs) | Last fill timestamp |
| `close_reason` | enum | See Q02 |
| `trace_id` | string | OTel root span |

**Execution Quality**

| Field | Notes |
|---|---|
| `entry_price_avg` | Volume-weighted |
| `exit_price_avg` | Volume-weighted |
| `entry_slippage_bps` | vs mid at signal fire |
| `exit_slippage_bps` | vs mid at close trigger |
| `fees_total_usd` | Exchange + network fees |
| `fill_duration_ms` | Signal → last fill |

**Risk Snapshot (at open)**

| Field | Notes |
|---|---|
| `initial_stop_price` | Exact value sent |
| `initial_tp_price` | May be null |
| `position_size_usd` | Notional |
| `leverage` | 1 for spot |
| `risk_reward_ratio` | At open time |

**Market Context**

| Field | Notes |
|---|---|
| `signal_mid_price` | Price when signal fired |
| `volatility_at_open` | e.g. 1h ATR |
| `spread_at_open_bps` | Orderbook spread |
| `funding_rate_at_open` | Perpetuals only |

**Outcomes**

| Field | Notes |
|---|---|
| `pnl_gross_usd` | Before fees |
| `pnl_net_usd` | After fees |
| `pnl_bps` | Normalized for cross-symbol comparison |
| `mae_usd` | Max adverse excursion |
| `mfe_usd` | Max favorable excursion |
| `exit_order_ids` | Array — links to exchange audit trail |

**Anti-pattern:** Storing only `pnl_net_usd`. Without MAE/MFE you cannot tell if a winner was lucky (price dipped 80% of stop then recovered) or clean.

---

## TFAI-Q02 — Closed `close_reason` Taxonomy & Versioning

### Design Principles

1. **Hierarchical dot-notation** — machine-parseable, SQL-friendly, human-readable.
2. **Immutable leaf codes** — never rename a code; deprecate with a successor.
3. **Version column** — tracks taxonomy schema version, not the value.

### Taxonomy (Starter)

```
exit.tp.full                   # All TP targets hit
exit.tp.partial                # Partial TP, position still live (see Q03)
exit.sl.initial                # Initial stop hit
exit.sl.trailing               # Trailing stop hit
exit.sl.time                   # Time-based stop (max hold duration)
exit.sl.drawdown               # Portfolio-level drawdown limit
exit.manual.operator           # Human override
exit.manual.risk_desk          # Risk desk intervention
exit.strategy.signal_reversal  # Strategy flipped signal direction
exit.strategy.regime_change    # Regime filter invalidated setup
exit.system.error.exchange     # Exchange returned unrecoverable error
exit.system.error.internal     # Internal exception / panic
exit.system.connectivity       # WebSocket/REST timeout, position emergency-closed
exit.system.position_sync      # Mismatch between local & exchange state, forced close
```

### Versioning Without Breaking Dashboards

```sql
-- Migration: add version column with default
ALTER TABLE positions ADD COLUMN close_reason_v INTEGER NOT NULL DEFAULT 1;

-- Compatibility view — dashboards point here, not raw table
CREATE VIEW v_positions AS
SELECT *,
  CASE
    WHEN close_reason_v = 1 AND close_reason = 'stop_loss' THEN 'exit.sl.initial'
    WHEN close_reason_v = 1 AND close_reason = 'take_profit' THEN 'exit.tp.full'
    ELSE close_reason
  END AS close_reason_canonical
FROM positions;
```

- Dashboards always query `close_reason_canonical` from the view.
- New codes only ever added, never modified.
- Maintain a `close_reason_registry` table: `(code, introduced_version, deprecated_version, successor_code, description)`.

**Anti-pattern:** Free-text `close_reason`. One engineer writes `"SL hit"`, another writes `"stop loss"`, a third writes `"sl"`. SQL `GROUP BY` is now useless.

---

## TFAI-Q03 — Partial TP + Full Close Under One `position_id` (Event Sourcing)

### Event Log Schema

```sql
CREATE TABLE position_events (
  event_id       TEXT PRIMARY KEY,   -- UUIDv7 (time-sortable)
  position_id    TEXT NOT NULL,
  event_type     TEXT NOT NULL,      -- see taxonomy below
  sequence_no    INTEGER NOT NULL,   -- monotonic per position
  occurred_at    INTEGER NOT NULL,   -- Unix μs
  exchange_order_id TEXT,
  qty_delta      REAL,               -- negative = reducing position
  qty_remaining  REAL,               -- denormalized for fast queries
  price          REAL,
  fee_usd        REAL,
  payload        TEXT                -- JSON for type-specific fields
);
CREATE UNIQUE INDEX idx_pos_seq ON position_events(position_id, sequence_no);
```

### Event Types

```
position.opened
position.tp_partial          -- partial take-profit fill
position.sl_moved            -- stop adjusted (trailing, breakeven)
position.tp_moved            -- target adjusted
position.hedge_added         -- if hedging supported
position.close_initiated     -- close order submitted
position.closed              -- final fill, qty_remaining = 0
position.error               -- exchange or system error mid-lifecycle
```

### Lifecycle Example

```
seq  event_type          qty_delta   qty_remaining   price
  1  position.opened     +1.000      1.000           65000
  2  position.tp_partial -0.333      0.667           67500   ← 33% TP
  3  position.sl_moved    null       0.667           65500   ← breakeven
  4  position.tp_partial -0.333      0.334           69000   ← 33% TP
  5  position.closed     -0.334      0.000           71000   ← final close
```

### Snapshot / Projection Pattern

Materialise a `positions` table as a **read-model** by replaying events:

```rust
fn apply_event(state: &mut PositionState, event: &PositionEvent) {
    match event.event_type.as_str() {
        "position.opened"     => state.qty_remaining = event.qty_delta.unwrap(),
        "position.tp_partial" => {
            state.qty_remaining += event.qty_delta.unwrap(); // delta is negative
            state.pnl_realised  += compute_pnl(event);
        }
        "position.closed" => {
            state.qty_remaining = 0.0;
            state.closed_at     = Some(event.occurred_at);
            state.close_reason  = extract_reason(&event.payload);
        }
        _ => {}
    }
}
```

**Benefits:**
- Full audit trail — replay any state at any point in time.
- Partial fills don't corrupt the aggregate.
- RCA can diff the event stream against exchange order history line-by-line.

**Anti-pattern:** Updating a single `positions` row in-place. When a bug causes a bad state transition, the pre-bug state is gone.

---

## TFAI-Q04 — Normalizing Exchange Error Codes

### The Problem

Binance alone has 100+ error codes. Raw codes in alerts produce noise; losing the code entirely makes debugging impossible.

### Normalization Schema

```rust
pub enum ErrorCategory {
    RateLimit,          // 429, 418
    AuthFailure,        // 1100, -2014, -2015
    InsufficientFunds,  // -2010, -1013
    InvalidOrder,       // -1102, -1111, -1116
    MarketClosed,       // -1013 with specific msg
    PositionRisk,       // -2019
    ExchangeInternal,   // -1000, -1001
    NetworkTransient,   // timeouts, 502, 503
    Unknown,
}

pub struct NormalizedError {
    pub raw_code:     i32,
    pub raw_message:  String,
    pub category:     ErrorCategory,
    pub retryable:    bool,
    pub retry_after_ms: Option<u64>,
    pub position_id:  Option<String>,
    pub exchange:     String,
    pub occurred_at:  i64,
    pub trace_id:     String,
}
```

### Normalization Table (Binance examples)

| Raw Code | Category | Retryable | Action |
|---|---|---|---|
| -1003 (rate limit) | `RateLimit` | yes | backoff per `Retry-After` header |
| -2010 (insufficient balance) | `InsufficientFunds` | no | alert + halt strategy |
| -1116 (invalid order type) | `InvalidOrder` | no | alert + code bug suspected |
| -1021 (timestamp out of sync) | `NetworkTransient` | yes | re-sync clock, retry |
| 429 | `RateLimit` | yes | respect header |
| 500–503 | `ExchangeInternal` | yes | exponential backoff |

### Alerting Tiers

```yaml
# PagerDuty / VictorOps severity mapping
critical:   [AuthFailure, InsufficientFunds, PositionRisk]
warning:    [RateLimit, InvalidOrder, ExchangeInternal]
info:       [NetworkTransient]  # if transient resolves < 30s
```

**Anti-pattern:** Alerting on raw code `-1003` with no human-readable label. On-call at 3am cannot act on an integer.

---

## TFAI-Q05 — Trace Topology for a Trade Lifecycle

A single root `trace_id` is necessary but not sufficient. You need a **span hierarchy** that survives async boundaries and exchange round-trips.

### Span Tree

```
trace_id: abc-123
│
├─ span: strategy.signal_evaluation          [signal_id, symbol, strategy_id]
│   └─ span: feature.compute                 [feature_set_version]
│
├─ span: risk.pre_trade_check                [check_version, result]
│
├─ span: order.submit (entry)                [order_id, venue]
│   ├─ span: http.post /order               [latency_ms, http_status]
│   └─ span: order.fill_wait                [fill_duration_ms]
│       └─ event: partial_fill              [fill_qty, fill_price]
│
├─ span: position.lifecycle                  [position_id]      ← long-lived
│   ├─ span: order.submit (partial_tp_1)    [order_id]
│   ├─ span: order.submit (partial_tp_2)    [order_id]
│   └─ span: order.submit (close)           [order_id, close_reason]
│
└─ span: position.settled                    [pnl_net_usd, fees_usd]
```

### Correlation IDs to Propagate

| ID | Propagated via | Scope |
|---|---|---|
| `trace_id` | OTel W3C header | Entire lifecycle |
| `signal_id` | DB column | Signal → position link |
| `position_id` | DB column + log field | All events for this trade |
| `order_id` | Exchange-assigned | Order ↔ fill correlation |
| `strategy_id` | Log field | Cross-position analysis |
| `session_id` | WS connection | Reconnect attribution |

### Long-Lived Span Problem

OTel exporters typically flush spans when they close. A position open for hours creates a span that never closes. Solutions:

1. Use a **link** from child spans back to the root `position.lifecycle` span rather than a parent reference.
2. Emit a heartbeat span every N minutes with position state.
3. Store `trace_id` in the DB — reconstruct the trace in your backend, don't rely on OTel keeping it hot.

---

## TFAI-Q06 — SLI Starter Set

SLIs for a trading system fall into three domains: **execution**, **data**, and **system health**.

### Execution SLIs

| SLI | Definition | Target |
|---|---|---|
| Order submission success rate | `successful_submissions / total_attempts` (5m window) | ≥ 99.5% |
| Fill latency P99 | Time from REST POST to first fill ack | ≤ 500ms |
| Slippage vs expected | `actual_fill - mid_at_signal` distribution | P95 ≤ 15bps |
| Stop execution rate | Orders with `close_reason=exit.sl.*` that actually closed within 2× ATR | ≥ 99.9% |

### Data SLIs

| SLI | Definition | Target |
|---|---|---|
| Market data freshness | Age of latest ticker per symbol | ≤ 500ms |
| Position sync accuracy | `|local_qty - exchange_qty| / local_qty` per reconciliation | ≤ 0.1% |
| Feature pipeline lag | Signal timestamp vs latest input bar timestamp | ≤ 1 bar |

### System Health SLIs

| SLI | Definition | Target |
|---|---|---|
| WebSocket uptime | Fraction of time WS connected per exchange | ≥ 99.9% |
| Reconciliation pass rate | Successful position reconciliation runs / scheduled runs | ≥ 99.8% |
| Error budget burn rate | (1 - SLI) consumed vs monthly budget | Alert at 5% in 1h |

**Checklist for each SLI:**
- [ ] Numerator and denominator are unambiguous
- [ ] Window (rolling vs calendar) is specified
- [ ] Alert threshold distinct from SLO target (alert before breach)
- [ ] SLI is queryable from production telemetry right now

---

## TFAI-Q07 — Sampling Strategy

### Tiered Approach

```
                     ┌─────────────────────┐
 Volume              │  HEAD-BASED SAMPLING │  1% of healthy, routine fills
 (noisy)             └─────────────────────┘
                              │
                     ┌─────────────────────┐
                     │  TAIL-BASED SAMPLING │  100% if span has error flag
 Precision           │  (error / anomaly)  │  100% if latency P99 exceeded
 (incidents)         └─────────────────────┘
                              │
                     ┌─────────────────────┐
 Always-on           │  FORCE-SAMPLE FLAG  │  position.lifecycle spans always
 (audit)             │                     │  exchange error spans always
                     └─────────────────────┘
```

### Rust Implementation Sketch

```rust
fn sampling_decision(span: &SpanContext) -> SamplingResult {
    // Always sample errors and slow spans
    if span.has_error || span.latency_ms > LATENCY_P99_THRESHOLD {
        return SamplingResult::RecordAndSample;
    }
    // Always sample trade-critical spans
    if matches!(span.name, "position.lifecycle" | "order.submit" | "position.settled") {
        return SamplingResult::RecordAndSample;
    }
    // Head-based 1% for everything else
    if thread_rng().gen::<f64>() < 0.01 {
        return SamplingResult::RecordAndSample;
    }
    SamplingResult::Drop
}
```

### Log Volume Controls

- **Structured logs only** — no `println!` style debug output in production.
- **Rate-limit per error code** — emit at most 1 log/sec for repeated identical errors; emit a `suppressed_count` field.
- **Separate high-volume streams** — orderbook tick data → time-series DB (InfluxDB / QuestDB), not your log aggregator.
- **Retention tiers** — hot 7 days (full), warm 30 days (sampled), cold 1 year (position-level only, no ticks).

**Anti-pattern:** Logging every orderbook update at INFO level. A 10-symbol feed at 100ms intervals = 864,000 log lines/day per symbol.

---

## TFAI-Q08 — Regime Change vs Software Bug

This is the most important analytical distinction in the system. Getting it wrong in either direction is expensive.

### Signal Matrix

| Observation | Suggests Bug | Suggests Regime |
|---|---|---|
| Win rate drop is sudden (single deployment) | ✓ | |
| All symbols degraded simultaneously | ✓ | |
| Specific symbol/exchange degraded | | ✓ |
| Slippage and latency metrics unchanged | | ✓ |
| Fill rates or order rejection rate changed | ✓ | |
| Degradation persists in paper trading too | | ✓ |
| Replay on historical data shows same behaviour | | ✓ |
| Replay shows degradation started before deploy | | ✓ |

### Statistical Approaches

**1. CUSUM (Cumulative Sum Control Chart)**
Detects when a performance metric shifts from its baseline mean. Sensitive to gradual drift — characteristic of regime change.

```python
def cusum(returns, k=0.5, h=5.0):
    """k = allowance, h = threshold"""
    pos, neg = 0.0, 0.0
    for r in returns:
        pos = max(0, pos + r - k)
        neg = max(0, neg - r - k)
        if pos > h or neg > h:
            return "SIGNAL"  # regime shift likely
    return "STABLE"
```

**2. Bayesian Changepoint Detection**
(e.g. `ruptures` library or custom BOCPD) — provides a posterior probability over changepoint locations. Aligns changepoints with deployment timestamps.

**3. Kolmogorov–Smirnov Test**
Compare the distribution of returns in window A (pre-event) vs window B (post-event). A significant KS p-value says the distributions differ; you then cross-reference with whether a deploy occurred.

**4. Permutation Test on Slippage**
If the bug is an execution bug (e.g. wrong order type), slippage distribution will shift independently of market regime. Test slippage before and after deploy using permutation (no normality assumptions required).

**Operational Checklist:**
- [ ] Run replay (see Q12) — if replay also degrades, it's regime
- [ ] Check if degradation started at exact deploy time (git blame + timestamp)
- [ ] Compare paper vs live simultaneously
- [ ] Check correlated assets — did similar uncorrelated strategies also degrade?

---

## TFAI-Q09 — FDR Control Across Symbols / Strategies

**Yes, absolutely.** Running the same hypothesis test across N symbols without correction inflates false positives linearly.

### Why It Matters

If you test 50 symbols at α=0.05, you expect ~2.5 false "this strategy is broken" signals even when nothing is wrong. Acting on those means halting profitable strategies unnecessarily.

### Recommended Approach: Benjamini–Hochberg (BH)

BH controls FDR (expected proportion of false discoveries among rejections) and is less conservative than Bonferroni:

```python
from statsmodels.stats.multitest import multipletests

p_values = [run_performance_test(sym) for sym in symbols]
reject, p_adj, _, _ = multipletests(p_values, alpha=0.05, method='fdr_bh')

degraded_symbols = [sym for sym, rej in zip(symbols, reject) if rej]
```

### Practical Tiers

| Use Case | Method | Rationale |
|---|---|---|
| Daily performance monitoring (50+ symbols) | BH-FDR | Balanced power vs FP control |
| Post-incident investigation (few hypotheses) | Bonferroni | Conservative, small N |
| Strategy comparison (correlated assets) | BY (Benjamini–Yekutieli) | Accounts for correlation |
| Continuous monitoring / streaming | Sequential probability ratio test (SPRT) | No fixed sample size |

**Anti-pattern:** Running 20 t-tests at p<0.05 and acting on any rejection. The risk team will eventually shut down a strategy that was simply unlucky, not broken.

---

## TFAI-Q10 — Data That Must Never Enter Production Logs

### Categories

**Personal / Identity Data (GDPR Art. 4)**
- Full API keys — log only last 4 chars or a masked hash
- Exchange account UIDs that map to individuals
- IP addresses of clients or counterparties (unless anonymised)
- Any KYC data that may pass through the system

**Financial Data with Regulatory Sensitivity**
- Raw order book snapshots if they could constitute market manipulation evidence (consult legal — jurisdiction-dependent)
- Aggregate position sizes above certain thresholds (may be reportable — MiFID II, CFTC)
- Counterparty-identifiable trade data in shared log systems

**Secrets**
- API secrets / HMAC signing keys — ever, in any form
- Database passwords, JWT secrets
- Private keys (if self-custody)

**Internal Strategy IP**
- Feature weights, model parameters, strategy logic in plaintext — these are trade secrets; logs may be subpoenaed or leaked

### Implementation Controls

```rust
// Scrubbing at log boundary
impl fmt::Display for ApiCredential {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "***{}", &self.key[self.key.len()-4..])
    }
}

// Deny-list in log pipeline (e.g. Vector/Fluent Bit)
[transforms.scrub_secrets]
  type = "remap"
  source = '''
  .message = replace(.message, r'(?i)(secret|api_key|password)\s*[:=]\s*\S+', "${1}=[REDACTED]")
  '''
```

**Checklist:**
- [ ] Log pipeline has automated PII scrubbing
- [ ] Secrets manager used — no secrets in env vars that appear in process dumps
- [ ] Log retention policy documented and enforced (GDPR right to erasure)
- [ ] Log access is role-gated (not all engineers can read production logs)
- [ ] Third-party log aggregators (Datadog, Grafana Cloud) reviewed for data residency

---

## TFAI-Q11 — Auditability of "AI Explained" Outputs

The core requirement: every AI-generated explanation must be traceable back to the exact data it reasoned over, so a human can verify or falsify the explanation independently.

### Schema for AI Explanation Records

```sql
CREATE TABLE ai_explanations (
  explanation_id   TEXT PRIMARY KEY,
  generated_at     INTEGER NOT NULL,        -- Unix μs
  model_id         TEXT NOT NULL,           -- e.g. "claude-sonnet-4-20250514"
  model_version    TEXT NOT NULL,
  prompt_hash      TEXT NOT NULL,           -- SHA-256 of exact prompt sent
  prompt_template_version TEXT NOT NULL,
  
  -- Source data references (not copies of data)
  position_ids     TEXT NOT NULL,           -- JSON array
  event_ids        TEXT NOT NULL,           -- JSON array of position_events.event_id
  log_query        TEXT,                    -- exact SQL / log query used
  log_query_result_hash TEXT,              -- SHA-256 of query results
  
  -- Output
  explanation_text TEXT NOT NULL,
  confidence_label TEXT,                    -- LOW / MEDIUM / HIGH
  supporting_facts TEXT NOT NULL,          -- JSON: [{claim, source_event_id, source_log_line}]
  
  -- Human review
  reviewed_by      TEXT,
  review_outcome   TEXT,                   -- CONFIRMED / DISPUTED / INCONCLUSIVE
  review_notes     TEXT
);
```

### Traceability Checklist

Each `supporting_facts` entry must contain:
- `claim` — the specific assertion the AI made
- `source_event_id` — the `position_events.event_id` that supports it (or null)
- `source_log_ref` — log timestamp + service + stream (so a human can pull the raw log)
- `source_metric_query` — the exact metric query + time range

### Example Output

```json
{
  "explanation_id": "exp-7f3a",
  "summary": "Stop not executed because WS feed was stale for 4.2 seconds at the critical moment.",
  "supporting_facts": [
    {
      "claim": "WebSocket feed went stale at 14:23:07.441 UTC",
      "source_event_id": "evt-9b21",
      "source_log_ref": "service=market-data ts=2024-01-15T14:23:07.441Z stream=prod-logs"
    },
    {
      "claim": "Stop order not submitted until 14:23:11.612 UTC",
      "source_event_id": "evt-9b29",
      "source_log_ref": "service=order-manager ts=2024-01-15T14:23:11.612Z"
    }
  ]
}
```

**Anti-pattern:** AI explanation stored as a narrative paragraph with no data references. When the finding is disputed 6 months later, there's nothing to verify against.

---

## TFAI-Q12 — Automated Checks When a Code Bug Is Suspected

### Checklist (in order)

**1. Regression Test Suite**
```bash
cargo test --all-features 2>&1 | tee test_output.txt
# Specifically: order execution, position accounting, stop logic
```

**2. Property-Based Tests**
Use `proptest` or `quickcheck` to fuzz edge cases (partial fills, zero quantity, fee rounding):
```rust
proptest! {
    #[test]
    fn pnl_calculation_never_negative_on_tp(
        entry in 1000f64..100000f64,
        exit in 1000f64..100000f64,
    ) {
        if exit > entry {
            assert!(compute_pnl(Side::Long, entry, exit) > 0.0);
        }
    }
}
```

**3. Event Log Replay**
Re-run the event stream from `position_events` through current code, compare output state to stored state:
```rust
fn replay_position(position_id: &str, events: &[PositionEvent]) -> PositionState {
    let mut state = PositionState::default();
    for event in events {
        apply_event(&mut state, event);
    }
    state
}
// Assert: replay_state == stored_final_state
```

**4. Shadow Mode Comparison**
Run the new build in shadow mode against live data, compare order decisions vs production without submitting:
```
prod_decision  = [BUY 0.1 BTC @ 65000 LIMIT]
shadow_decision= [BUY 0.1 BTC @ 65000 LIMIT]  ✓
# On divergence: emit alert + capture full state diff
```

**5. Git Bisect + Deployment Diff**
```bash
git bisect start
git bisect bad HEAD
git bisect good <last_known_good_sha>
# Run performance regression test at each step
git bisect run ./scripts/perf_regression_test.sh
```

**6. Exchange Order History Reconciliation**
Pull raw order history from exchange API for the incident window, compare against local `position_events`:
```python
exchange_fills = fetch_exchange_fills(start=incident_start, end=incident_end)
local_fills = query_db("SELECT * FROM position_events WHERE event_type LIKE '%fill%'")
diff = reconcile(exchange_fills, local_fills)
# Any unmatched fills = data integrity bug
```

**7. Static Analysis**
```bash
cargo clippy -- -D warnings
cargo audit  # dependency vulnerabilities
```

---

## TFAI-Q13 — Postmortem Template for Trading Incidents

```markdown
# Incident Postmortem — [INCIDENT-ID]

**Date:** YYYY-MM-DD  
**Severity:** P1 / P2 / P3  
**Duration:** HH:MM (detection → mitigation)  
**Author(s):**  
**Review Status:** Draft / Under Review / Final  

---

## 1. Executive Summary (3 sentences max)
What happened, what was the financial/operational impact, what was the root cause.

---

## 2. Impact
- **P&L impact:** $X gross, $Y net (after fees)
- **Positions affected:** N positions, symbols: [list]
- **Duration of degraded state:** 
- **SLOs breached:** [list SLIs that fell below target]
- **Customer / regulatory impact:** [if applicable]

---

## 3. Timeline (UTC)
| Time | Event |
|------|-------|
| HH:MM | First anomaly observable in metrics |
| HH:MM | Alert fired |
| HH:MM | On-call acknowledged |
| HH:MM | Mitigation action taken |
| HH:MM | Trading halted / positions closed |
| HH:MM | Root cause identified |
| HH:MM | Full resolution |

---

## 4. Root Cause
[Single precise statement. Reference specific event_ids, log lines, commit SHAs.]

## 5. Contributing Factors
- [Factor 1: e.g. "No alerting on WS staleness > 2s"]
- [Factor 2]

---

## 6. What Went Well
- [e.g. "Circuit breaker halted new entries within 30s"]

## 7. What Went Poorly
- [e.g. "SL not executed for 4.2s due to stale feed — no alert existed"]

---

## 8. Action Items
| Action | Owner | Due | Status |
|--------|-------|-----|--------|
| Add WS staleness alert (< 1s threshold) | @eng | +3d | Open |
| Add replay test for WS-dropout scenario | @eng | +5d | Open |
| Review stop-execution SLI definition | @risk | +7d | Open |

---

## 9. Lessons Learned
[Max 5 bullets. Focus on systemic issues, not individual blame.]

---

## 10. Appendix
- Relevant trace IDs: [list]
- Position IDs: [list]
- AI explanation record: [explanation_id]
- Grafana dashboard link (snapshot): [url]
```

**Process notes:**
- Draft within 24h of resolution while memory is fresh.
- Review meeting ≤ 48h, no longer.
- Blameless by policy — name systems and decisions, not individuals.
- Archive all postmortems in a searchable repo; search before each new postmortem.

---

## TFAI-Q14 — Ownership, Governance, and Approval Flow

### Who Owns What

| Domain | Owner | Rationale |
|---|---|---|
| Telemetry infrastructure (collectors, pipelines, storage) | Engineering (SRE/Platform) | Operational expertise |
| SLI/SLO definitions | Engineering + Risk jointly | Technical feasibility meets risk appetite |
| Alert thresholds | Risk (approves), Engineering (implements) | Risk sets the bar |
| Postmortem process | Engineering lead | Operational cadence |
| AI explanation audit trail | Risk / Compliance | Regulatory exposure |
| Log access control | Security / Compliance | Data governance |
| Production change approval | Risk gate-keeps; Engineering implements | Four-eyes minimum |

### Production Change Approval Flow

```
Developer
  │
  ├─ Opens PR with:
  │   - Change description
  │   - Test evidence (unit, replay, shadow)
  │   - SLI impact assessment
  │   - Rollback procedure
  │
  ▼
Peer Review (Engineering)
  │  ≥ 1 approval from senior engineer
  ▼
Risk Review (Risk Desk)
  │  Required for:
  │    - Any change to position sizing, stop logic, execution
  │    - Alert threshold changes
  │    - New exchange integrations
  │  Not required for:
  │    - Dashboard-only changes
  │    - Test additions
  │    - Non-execution observability
  ▼
Staging Deployment + Shadow Mode (≥ 24h)
  │
  ▼
Risk Sign-off on Staging Metrics
  │
  ▼
Production Deployment (business hours only, on-call notified)
  │
  ▼
Post-deploy monitoring window (≥ 2h, deployer stays on)
```

### Separation of Duties (Hard Rules)

- The engineer who writes execution logic **cannot** be the sole approver of their own risk-related change.
- Risk desk has **read access** to all production observability but **cannot push code**.
- Emergency rollback (position halt, circuit breaker) can be triggered by risk desk **without engineering approval**.
- All production access is logged and reviewed monthly.

**Anti-pattern:** Engineering owns everything including its own risk thresholds. This creates a conflict of interest — the team under pressure to ship is also the team setting the safety limits.

---

*All 14 questions answered. No investment advice provided.*
