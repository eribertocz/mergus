---
id: quality-evaluation
title: How tests run, monitors, profiles and how quality travels
crate: mergus-quality
phase: "0.5"
status: draft
depends_on: [quality-tests]
---

# Evaluation

### MG-REQ-0811 · Before publication

Tests MUST run in stage 6 on candidates, before the adapter commit, so that a failing version
is never visible.

- Kind: behavior
- Verify: test `mg_req_0811_tests_before_commit`

### MG-REQ-0812 · One pass per table

All tests of a table MUST be compiled into one query that reads the data once and counts the
violations of each test.

- Kind: behavior
- Verify: test `mg_req_0812_single_pass_per_table`

### MG-REQ-0813 · Metrics even when blocked

Test results and counts MUST be stored in `ops.quality` even when the publication is blocked.

- Kind: behavior
- Verify: test `mg_req_0813_metrics_stored_when_blocked`

### MG-REQ-0814 · Incremental evaluation

For incremental models, row-level tests MUST evaluate only changed periods; `unique` and
`relationship` MUST compare new rows with existing ones.

- Kind: behavior
- Verify: test `mg_req_0814_incremental_tests_on_changed_periods`

### MG-REQ-0815 · Failing rows sample

Up to a configurable number of failing rows per test (default 100) MUST be stored in the ops
zone, never in gold, with `pii` columns pseudonymized.

- Kind: behavior
- Verify: test `mg_req_0815_failing_rows_sampled_in_ops`

### MG-REQ-0816 · Errors that explain

A blocking failure MUST print the test, the model, failing and total row counts, up to three
example keys, and which published version stays visible.

- Kind: behavior
- Verify: test `mg_req_0816_failure_message_contents`

### MG-REQ-0817 · Automatic monitors

Every published table MUST get, without configuration, a volume monitor and a freshness
monitor based on run history, acting as `warn`.

- Kind: behavior
- Verify: test `mg_req_0817_volume_and_freshness_monitors`

### MG-REQ-0818 · Column profile

In the same pass, Mergus MUST compute per column: null ratio, approximate distinct count,
minimum and maximum, stored in `ops.profiles`.

- Kind: behavior
- Verify: test `mg_req_0818_profile_in_same_pass`

### MG-REQ-0819 · Quality travels with the data

The manifest MUST carry, per table, passed tests and active warnings. A warning MUST propagate
through lineage to downstream gold tables and MUST clear when a later run passes.

- Kind: behavior
- Verify: test `mg_req_0819_warning_propagates_and_clears`
