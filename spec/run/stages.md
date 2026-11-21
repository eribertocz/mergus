---
id: run-stages
title: The six stages of a run, failure semantics and resuming
crate: mergus-runtime
phase: "0.4"
status: draft
depends_on: [architecture-invariants]
---

# Stages

```
1 Read ─► 2 Resolve ─► 3 Plan ─► 4 Verify ─► 5 Execute ─► 6 Publish
└──────────── no data touched ───────────┘   └──── with data ────┘
```

| Stage | Input | Output | Crate |
|---|---|---|---|
| 1 Read | Project files | Read project in canonical form | `mergus-project` |
| 2 Resolve | Read project, current catalog, Menura documents, adapter capabilities, source schemas | Resolved project: references, types, levels | `mergus-plan` |
| 3 Plan | Resolved project, previous fingerprints, input versions | Plan | `mergus-plan` |
| 4 Verify | Plan | Approved plan or errors (`run/gates.md`) | `mergus-plan` |
| 5 Execute | Approved plan | Candidates in work | `mergus-runtime` |
| 6 Publish | Candidates | New gold version, catalog, manifest, pointer | `mergus-runtime` |

### MG-REQ-0701 · Stages have defined inputs and outputs

Each stage MUST be a function from its input to its output as in the table, testable alone.
Stages 1 to 4 MUST NOT read or write table data; stage 2 MAY read metadata only.

- Kind: behavior
- Verify: test `mg_req_0701_stages_one_to_four_touch_no_data`

### MG-REQ-0702 · Read finds every error at once

Stage 1 MUST report every error it finds in one pass, grouped by file, not stop at the first.

- Kind: behavior
- Verify: test `mg_req_0702_all_errors_reported_together`

### MG-REQ-0703 · No cycles

A cycle between models MUST be an error listing the models of the cycle in order.

- Kind: behavior
- Verify: test `mg_req_0703_cycle_reported_in_order`

### MG-REQ-0704 · Execute follows the plan

Stage 5 MUST execute the approved plan exactly, taking no new decisions. Extraction runs
first, then segments in topological order with independent branches in parallel.

- Kind: behavior
- Verify: test `mg_req_0704_execution_matches_plan`

### MG-REQ-0705 · Candidates are invisible

Results of stage 5 MUST be stored in work as candidates identified by fingerprint, invisible
outside the run.

- Kind: behavior
- Verify: test `mg_req_0705_candidates_invisible`

### MG-REQ-0706 · Failure semantics

A failure in stages 1–4 MUST leave everything unchanged. A failure in stage 5 MUST leave gold,
catalog and manifests unchanged and keep complete candidates for resuming. A failure in stage
6 before the pointer moves MUST leave the previous manifest current. The pointer move itself
is atomic.

- Kind: behavior
- Verify: test `mg_req_0706_failure_at_each_stage`
- Phase: 0.5
- Depends: MG-REQ-0102

### MG-REQ-0707 · Resume by fingerprint

A retried run MUST compute a new plan and reuse every candidate in work and every committed
raw load whose fingerprint or `load_id` matches, so that finished work is not repeated.

- Kind: behavior
- Verify: test `mg_req_0707_retry_reuses_finished_work`
- Depends: MG-REQ-0143

### MG-REQ-0708 · Cancellation

On cancellation, Mergus MUST stop segments, close readers and connections, keep candidates
useful for resuming, delete other temporary files, and exit with code 7.

- Kind: behavior
- Verify: test `mg_req_0708_cancel_cleans_and_exits_7`

### MG-REQ-0709 · Finalizer always runs

Whether a run succeeds or fails, Mergus MUST clean work files not useful for resuming, write
the run report and send notifications.

- Kind: behavior
- Verify: test `mg_req_0709_finalizer_runs_on_failure`

### MG-REQ-0718 · Run limits

A run MUST accept limits on wall time, rows written and bytes scanned, from `project.toml` or
the command line, the stricter one winning. Before a limit is exceeded the run MUST stop the
step, publish nothing, and end with a typed error naming the limit.

- Kind: behavior
- Verify: test `mg_req_0718_run_stops_at_limit_without_publishing`
- Decision: 0010
