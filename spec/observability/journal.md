---
id: observability-journal
title: run_id, journal first, retention and cost
crate: mergus-ops
phase: "0.7"
status: draft
depends_on: [contracts-ops-tables]
---

# Journal

### MG-REQ-1101 · run_id ties everything

Every run MUST have a `run_id` that is also its OpenTelemetry trace id, carried by every log
line, metric, lineage event, report, manifest and alert of the run.

- Kind: behavior
- Verify: test `mg_req_1101_run_id_everywhere`

### MG-REQ-1102 · Journal first

Every event MUST be appended immediately to a local append-only journal (JSON Lines, one
file per run) before anything else is done with it. The finalizer loads the journal into
the ops tables. A journal left by a crashed run MUST be loaded at the start of the next run.

- Kind: behavior
- Verify: test `mg_req_1102_journal_survives_crash`

### MG-REQ-1103 · Logs carry metadata only

The internal logging API MUST accept only metadata (names, counts, durations, codes), never
row values. The only exception is the failing-row sample of MG-REQ-0815.

- Kind: behavior
- Verify: test `mg_req_1103_log_api_rejects_row_values`

### MG-REQ-1104 · Two-tier retention

Ops detail MUST be kept 30 days and a daily summary (durations, rows, success rate, quality
per table) 13 months by default, both configurable.

- Kind: behavior
- Verify: test `mg_req_1104_ops_retention_tiers`

### MG-REQ-1105 · Observability costs under two percent

Journal, ops tables, profiles and telemetry together MUST add less than 2% to the duration
of the reference benchmark run.

- Kind: process
- Verify: check `benchmarks`

### MG-REQ-1106 · Run report

Every run MUST produce a report with `run_id`, `plan_id`, start and end, per model (skipped,
full, which periods, rows, duration), boundaries, gates passed, test results, the published
manifest or the reason for not publishing, and the failing-row samples.

- Kind: behavior
- Verify: test `mg_req_1106_run_report_contents`

### MG-REQ-1107 · Actor in the journal and the audit chain

The journal and the hash-chained audit entries (MG-REQ-1054) MUST record the `actor` of
MG-REQ-0264 when one is set, with its `id`, `kind` and `on_behalf_of`, under the same trust rule.

- Kind: behavior
- Verify: test `mg_req_1107_audit_records_trusted_actor`
- Depends: MG-REQ-0264, MG-REQ-1054
- Decision: 0008, 0015
