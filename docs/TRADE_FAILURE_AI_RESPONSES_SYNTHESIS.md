# TFAI external answers — synthesis (IQAI mapping)

**Sources:**

1. **ChatGPT** — user batch `TFAI-Q01` … `TFAI-Q14`.
2. **Gemini** — user batch (same keys).
3. **DeepSeek** — user batch (same keys).
4. **Claude** — user batch (same keys; very detailed schemas / runbooks).

**Purpose:** Translate generic architecture advice into **IQAI repo reality** and **backlog ordering**. Not a substitute for code review.

---

## Parseability

Paste answers to Cursor with **`[TFAI-Qnn]`** blocks. Use **`TFAI-SOURCE: chatgpt` | `gemini` | `deepseek` | `claude`** so synthesis can track provenance.

---

## Crosswalk: recommendation → IQAI today → next step

| Key | External theme | IQAI / repo today (high level) | Suggested direction |
|-----|----------------|--------------------------------|---------------------|
| **Q01** | Full RCA field set | `positions` + `TradeLog`; snapshots | **μs timestamps**, VWAP entry/exit, **spread_at_open**, **funding** (perp), MAE/MFE, `exit_order_ids[]`, risk snapshot at open |
| **Q02** | Versioned `close_reason` | Free-text | **Dot-notation** (`exit.sl.trailing`); **`close_reason_registry`** + **`close_reason_v`**; **SQL view** `close_reason_canonical` for dashboards; never rename leaves |
| **Q03** | Event sourcing | `TradeEvent` + DB | **`position_events`** (UUIDv7, `sequence_no`, `payload` JSON); replay → `apply_event`; **Claude** example lifecycle table |
| **Q04** | Normalize errors | `iqai-binance` | **`ErrorCategory` Rust enum** + `NormalizedError` struct; **alert tiers** (critical/warning/info); mapping table with retry |
| **Q05** | Tracing | Logs | **Span tree** (strategy → risk → order.submit → position.lifecycle); **long-lived span** problem → **links** + heartbeat / DB `trace_id` |
| **Q06** | SLIs | Ad-hoc | **Targets** in doc (e.g. submit ≥99.5%); **feature pipeline lag**; **error budget burn**; position sync |
| **Q07** | Sampling | `RUST_LOG` | **Rust** `sampling_decision` sketch; **rate-limit duplicate error codes** + `suppressed_count`; ticks → TSDB not logs |
| **Q08** | Regime vs bug | Informal | **Signal matrix** (deploy vs paper vs replay); **KS + permutation on slippage**; Bayesian changepoint + CUSUM (all models) |
| **Q09** | FDR | N/A | BH; **Benjamini–Yekutieli** if assets correlated; **SPRT** for streaming |
| **Q10** | Safe logging | Hygiene | **Strategy IP** (weights, params) not in plaintext logs; **Vector remap** scrub; **log access** role-gated; residency |
| **Q11** | Auditable AI | Ollama | **`ai_explanations` table**: `prompt_hash`, `log_query_result_hash`, `supporting_facts` JSON, human **review_outcome** |
| **Q12** | Automation | T-05 + tests | **proptest**; **shadow mode** vs prod; **git bisect** + perf script; **exchange vs local fill diff**; `clippy` / `cargo audit` |
| **Q13** | Postmortem | None | **Claude** full markdown template (exec summary, SLO breach, blameless, appendix) |
| **Q14** | Ownership | Implicit | Eng/Risk **joint SLI**; **Risk approves** thresholds; **staging + shadow ≥24h**; **emergency rollback** by Risk without eng; **separation of duties** |

---

## Model-specific emphasis (deltas)

### Gemini (selected)

| Area | Nuance |
|------|--------|
| Q01 | signal vs execution latency; don’t clobber open row without history |
| Q05 | `position_id` long-lived; **baggage** |
| Q08 | Two signatures: infra correlated vs flat |
| Q14 | Dual-key execution changes |

### DeepSeek (selected)

| Area | Nuance |
|------|--------|
| Q01 | MAE/MFE, **`strategy_run_id`** |
| Q04 | Three-layer YAML + message pattern |
| Q08 | CUSUM, Kaplan–Meier, bimodal |
| Q11 | Evidence bag + **log line hash** |
| Q12 | Bug forensics; idempotency flood |

### Claude (selected)

| Area | Nuance |
|------|--------|
| **Q01** | Tables: identity, execution quality, **risk snapshot at open**, market context, outcomes; anti-pattern: **only** `pnl_net` |
| **Q02** | **`exit.*` dot hierarchy**; `close_reason_registry` with successor; **compatibility VIEW** for migrations |
| **Q03** | Full **`position_events` DDL** + event names + **projection** `apply_event` Rust sketch |
| **Q04** | **`NormalizedError` Rust** + Binance table + **PagerDuty tier YAML** |
| **Q05** | **ASCII span tree**; **long-lived OTel span** problem + mitigations (links, heartbeat, store trace in DB) |
| **Q06** | SLIs with **numeric targets**; **data** vs **execution** vs **system** domains; **error budget burn** |
| **Q07** | Tiered diagram + **Rust sampling**; **per-error code rate limit**; retention tiers |
| **Q08** | **Signal matrix** table (bug vs regime); CUSUM snippet; KS + **permutation on slippage**; operational checklist |
| **Q09** | **BY** when correlated; **SPRT** for streaming |
| **Q10** | **Strategy IP** in logs = subpoena risk; **Vector/Fluent Bit** scrub example |
| **Q11** | Full **`ai_explanations` schema** + JSON example with `supporting_facts` |
| **Q12** | Ordered checklist: tests → proptest → **replay** → **shadow** → bisect → **exchange reconcile** → clippy/audit |
| **Q13** | Publishable **postmortem markdown** + process (24h draft, 48h review, searchable archive) |
| **Q14** | **Ownership table**; **approval flow** diagram; **hard rules** (risk emergency halt, no self-approval) |

**Consensus across four LLMs:** events + taxonomy + normalized errors + FDR + no secrets + auditable AI + governance. **Claude** adds the most **implementation-shaped** artifacts (SQL, Rust, matrices, templates).

---

## Themes (condensed, all sources)

1. **Truth = events + fills**, not only a mutable `positions` row.
2. **Semantics:** stable `close_reason` (dot or `CAT:SUB`) + registry + version column + views.
3. **Exchange errors:** canonical category for alerts; raw for forensics.
4. **IDs:** nested spans; `position_id` as correlation; OTel long-span limitations acknowledged.
5. **Sampling:** tiered + rate-limited duplicates; critical spans always on.
6. **Regime vs bug:** signal matrix + statistics (CUSUM / KS / permutation / replay).
7. **Many symbols:** FDR; BY vs BH depending on correlation.
8. **AI explanations:** structured rows + hashes + evidence facts (Claude schema ≈ DeepSeek evidence bag).
9. **Governance:** joint Risk/SRE; emergency actions; separation of duties.

---

## Beyond more chatbots: who else to ask?

You already have **four general-purpose LLMs** on the same TFAI set — diminishing returns for *generic* architecture. Consider **non-LLM** or **domain experts** instead of a “fifth AI”:

| Consultant | Why |
|------------|-----|
| **SRE / platform engineer** (human) | Validate OTel cost, retention, on-call runbooks |
| **Exchange integration engineer** | Binance-specific edge cases (rate limits, recvWindow) |
| **Risk / compliance** (human) | Q10/Q14: jurisdiction, MiFID/CFTC, what must be in audit trails |
| **Quant / statistician** | Q08/Q09: regime change, FDR on *correlated* crypto returns |
| **Security** | Log pipeline redaction, secret handling, access review |

If you still want **another model**, use it for **narrow** follow-ups (e.g. “write `proptest` cases for partial fill rounding”) rather than repeating the full TFAI-14.

---

## Alignment with existing IQAI docs

| Doc | Relation |
|-----|----------|
| `TRADE_FAILURE_ANALYSIS.md` | Roadmap P0–P4 |
| `BACKTEST_TRADE_MANAGEMENT.md` | Q12 replay — T-05 parity |
| `DEV_TO_PROD_DEPLOY.md` | Q14 staging / approval |
| `API_ERRORS.md` | Q04 — error shape |
| `TELEGRAM_Q_ANALIZ_SPAM.md` | Q11 — AI + snapshot |
| `TRADE_FAILURE_TFAI_CLAUDE_FULL.md` | Claude’un tam TFAI metni (tek dosya arşiv) |
| `TRADE_FAILURE_PROGRESS.md` | Uygulama durumu / tamamlanan maddeler |
| `POSTMORTEM_TEMPLATE.md` | Q13 postmortem şablonu |
| `ALERT_TIERS.md` | Q04 `AlertTier` eşlemesi |
| `OPERATIONS_GOVERNANCE.md` | Q14 özet süreç |

---

## What *not* to do blindly

- Full event-sourcing rewrite without incremental `execution_events` / `position_events` pilot.
- Storing every raw fill forever without retention policy.
- Letting LLM “root cause” replace DB + logs (see §7 in `TRADE_FAILURE_ANALYSIS.md`).
- Selecting only top-performing symbols from a large universe **without** multiple-testing awareness (Q09).
- **CUSUM/KM** on thin data: need enough sample size before acting.
- Copying **Claude’s SLO targets** (e.g. 99.5%) without measuring your current baseline first.

---

*Last updated: ChatGPT + Gemini + DeepSeek + Claude TFAI-Q01–Q14 batches.*
