# Trade failure analysis — English prompt pack (for external AIs)

This file is the **English** companion to `docs/TRADE_FAILURE_ANALYSIS.md` §8. Use it to query ChatGPT, Claude, Gemini, or human consultants. Answers can be pasted back into Cursor with the **response keys** below so the coding assistant can map replies to questions.

---

## How to use (user)

1. Copy **§ Master prompt** (below) into your AI tool, **or** ask questions one-by-one using **§ Per-question prompts**.
2. When pasting answers back to Cursor, **prefix each block** with the key `TFAI-Qnn` (see **Response key scheme**).
3. Optional: add one line at the top: `TFAI-BATCH: v1` so tools know which template version you used.
4. Optional: `TFAI-SOURCE: chatgpt` | `gemini` | `deepseek` | `claude` so synthesis can track provenance.

---

## Response key scheme (for Cursor / assistants)

| Key | Maps to | Original § |
|-----|---------|------------|
| `TFAI-Q01` | Minimum field set for closed-position root-cause | 8.1 #1 |
| `TFAI-Q02` | `close_reason` closed taxonomy & versioning | 8.1 #2 |
| `TFAI-Q03` | Event sourcing: partial TP vs full close under `position_id` | 8.1 #3 |
| `TFAI-Q04` | Normalizing exchange error codes (Binance, etc.) | 8.1 #4 |
| `TFAI-Q05` | Trace ID / spans for trade lifecycle | 8.2 #5 |
| `TFAI-Q06` | SLI-style metrics | 8.2 #6 |
| `TFAI-Q07` | Log sampling strategy | 8.2 #7 |
| `TFAI-Q08` | Statistics: regime change vs bug | 8.3 #8 |
| `TFAI-Q09` | FDR / multiple comparisons across symbols | 8.3 #9 |
| `TFAI-Q10` | Data never to log in production | 8.4 #10 |
| `TFAI-Q11` | Auditable “AI explained” outputs | 8.4 #11 |
| `TFAI-Q12` | Automated checks when code is suspected | 8.5 #12 |
| `TFAI-Q13` | Postmortem template | 8.5 #13 |
| `TFAI-Q14` | Ownership: engineering vs risk; approval flow | 8.6 #14 |

**Paste format (recommended):**

```text
TFAI-BATCH: v1

[TFAI-Q01]
<your answer paragraphs>

[TFAI-Q02]
<your answer>
```

Alternative one-liner prefix:

```text
TFAI-Q05: OpenTelemetry is sufficient if ...
```

---

## Master prompt (single message)

Copy everything inside the box:

```text
You are a senior SRE / trading-systems architect. Answer the following 14 questions about designing observability and root-cause analysis for an automated crypto trading stack (Rust, SQLite, exchange REST/WebSocket). Be concrete; where useful, give examples, anti-patterns, and a short checklist. Do NOT give investment advice or trade recommendations.

Number your answers TFAI-Q01 through TFAI-Q14 matching the headings below.

--- TFAI-Q01 (data model) ---
What is the minimum set of fields that must be present on a CLOSED position to perform serious root-cause analysis (beyond “we lost money”)?

--- TFAI-Q02 (taxonomy) ---
How would you design a CLOSED taxonomy for `close_reason`, and how would you version / extend it without breaking SQL dashboards?

--- TFAI-Q03 (event sourcing) ---
How should partial take-profit and full close be modeled under the same `position_id` using an event-sourcing style?

--- TFAI-Q04 (exchange errors) ---
How should exchange error codes (e.g. Binance) be normalized for alerting and reporting?

--- TFAI-Q05 (tracing) ---
Is a single OpenTelemetry-style trace_id enough for a trade lifecycle? Which spans or child correlations would you add?

--- TFAI-Q06 (SLIs) ---
Which metrics would you define as SLIs (e.g. successful order submission rate)? List a small starter set.

--- TFAI-Q07 (sampling) ---
What sampling strategy would you use to prevent log-volume explosions while preserving incident debuggability?

--- TFAI-Q08 (statistics) ---
Which statistical approaches help distinguish a low win rate due to REGIME CHANGE vs a software BUG?

--- TFAI-Q09 (multiple testing) ---
Should false discovery rate (FDR) control be considered when comparing many symbols or strategies?

--- TFAI-Q10 (compliance) ---
Which categories of data should NEVER be stored in production logs (regulation / GDPR-style thinking)?

--- TFAI-Q11 (auditability) ---
How can “AI explained” outputs be made auditable (references to source log lines / IDs)?

--- TFAI-Q12 (automation) ---
What automated checks would you run when a “code bug” is suspected (tests, replay, diff)?

--- TFAI-Q13 (postmortem) ---
Outline a concise postmortem template for trading incidents.

--- TFAI-Q14 (ownership) ---
Who should own this observability stack—engineering vs risk—and what should the approval flow look like for production changes?
```

---

## Per-question prompts (copy individually)

### TFAI-Q01 — Minimum fields (closed position)

```text
(Context: automated crypto trading, SQLite + Rust.) What is the minimum set of fields that must be present on a CLOSED position to perform serious root-cause analysis? List required vs optional fields and why.
```

### TFAI-Q02 — `close_reason` taxonomy

```text
Design a closed taxonomy for `close_reason` for a trading system. Explain versioning, migration, and how to keep SQL dashboards stable when adding new reasons.
```

### TFAI-Q03 — Event sourcing (partial vs full)

```text
Model partial take-profit and full close under one `position_id` using event sourcing. What events, ordering guarantees, and idempotency keys would you use?
```

### TFAI-Q04 — Exchange error normalization

```text
How should exchange API error codes (e.g. Binance futures) be normalized into stable internal codes for metrics and alerts?
```

### TFAI-Q05 — Trace / spans

```text
For one trade lifecycle (signal → orders → partials → close), is one trace_id enough? Specify recommended spans and attributes.
```

### TFAI-Q06 — SLI metrics

```text
List 5–10 SLI-style metrics you would track for a retail/systematic trading bot. Include at least one latency and one reliability metric.
```

### TFAI-Q07 — Log sampling

```text
Propose a sampling policy for INFO vs ERROR logs in a high-frequency polling system so that incidents remain debuggable without unbounded storage.
```

### TFAI-Q08 — Regime vs bug

```text
How would you statistically or procedurally distinguish poor performance due to market regime change from a software defect? Name specific tests or procedures where possible.
```

### TFAI-Q09 — FDR / many symbols

```text
When monitoring many symbols in parallel, should we apply false discovery rate (FDR) or similar corrections? When is it overkill?
```

### TFAI-Q10 — Never log

```text
List categories of data that must not appear in production logs for a trading system. Be specific (examples: secrets, full payloads, etc.).
```

### TFAI-Q11 — Auditable AI explanations

```text
How can natural-language “explanations” generated by an LLM remain auditable? Reference immutable source records (log IDs, trade IDs, snapshot hashes).
```

### TFAI-Q12 — Automated suspicion of code bug

```text
If engineers suspect a code bug (not market noise), what automated pipeline would you run (unit tests, replay, canary, binary diff)? Order the steps.
```

### TFAI-Q13 — Postmortem template

```text
Provide a short postmortem template (sections + prompts) for a production trading incident. Keep it under one page.
```

### TFAI-Q14 — Ownership & approvals

```text
Who should own observability for trading: core engineering, quant/risk, or SRE? Describe a sensible approval flow for config changes that affect risk.
```

---

## Disclaimer

Do not treat LLM answers as production-ready designs without human review and code audit. This pack is for **discovery and documentation** only.

---

## Source

Derived from `docs/TRADE_FAILURE_ANALYSIS.md` §8. Cross-link: Turkish narrative and IQAI-specific tables remain in `TRADE_FAILURE_ANALYSIS.md`.

## After you receive answers

Paste batches into Cursor with `[TFAI-Qnn]` keys. A **repo synthesis** (mapping recommendations → IQAI backlog) is maintained in **`docs/TRADE_FAILURE_AI_RESPONSES_SYNTHESIS.md`** — update that file when you add new external answers. That file also has **§ Beyond more chatbots** (when to stop asking LLMs and involve humans: SRE, risk, quant, security).

**Full single-file technical reference (Claude, TFAI-Q01–Q14):** `docs/TRADE_FAILURE_TFAI_CLAUDE_FULL.md` — SQL/Rust örnekleri, span ağacı, SLI tabloları, postmortem şablonu.
