---
id: cli-output
title: Output modes and how errors are written
crate: mergus-cli
phase: "0.2"
status: draft
depends_on: [contracts-events]
---

# Output

| Context | Output |
|---|---|
| Interactive terminal | Live tree per model with rows, duration and level; final summary of what changed for dashboards |
| No terminal (CI, logs) | Plain lines with time and `run_id`, no colors or animation |
| `--json` | Events only (`contracts/events.md`) |

### MG-REQ-1321 · Mode detection

Mergus MUST choose the interactive or plain mode by detecting a terminal on standard output,
and MUST honour `NO_COLOR`, `--quiet` and `--verbose`.

- Kind: behavior
- Verify: test `mg_req_1321_output_mode_detection`

### MG-REQ-1322 · Errors that teach

Every error MUST show: its code, what happened in one sentence, the file, line and column
with the code underlined, why it matters, how to fix it with a concrete example, and how to
get the long explanation. Wording MUST be neutral and never blame the user.

```
error[MG-1042]: column "amount" would lose precision
  --> models/gold/sales/fact_sales.py:14:22
   |
14 |     return p1.with_columns(pl.col("amount").cast(pl.Float64))
   |                            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ DECIMAL(18,2) → Float64
   = why: Float64 cannot hold every decimal exactly; totals may not add up.
   = fix: keep the decimal, e.g. pl.col("amount").cast(pl.Decimal(18, 2))
   = more: mergus explain MG-1042
```

- Kind: behavior
- Verify: test `mg_req_1322_error_layout`

### MG-REQ-1323 · All errors at once

When several errors exist, Mergus MUST show all of them grouped by file in one run.

- Kind: behavior
- Verify: test `mg_req_1323_errors_grouped_by_file`

### MG-REQ-1324 · Structured diagnostics

Every error and warning MUST also exist as a structured diagnostic (code, severity, file,
line, column, suggestions), emitted as a `diagnostic` event with `--json`.

- Kind: behavior
- Verify: test `mg_req_1324_diagnostics_structured`

### MG-REQ-1325 · The name lives in one constant

The product name MUST be defined once at build time and used for the binary, help,
completions, the `MERGUS_` variable prefix and messages.

- Kind: process
- Verify: review `name-constant`
