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

Stage 1 MUST report every error it finds in one pass, grouped by file, not stop at the first:

1. MG-REQ-0702.1: the groups follow the layout order of the files: `project.toml`, `ids.toml`,
   `sources/`, `models/`, `macros/`, `lib/`, `seeds/`, `contracts/`, then any other file, as
   the tree of `project/layout.md` lists them. The errors of one file keep the order in which
   the file states them.

- Kind: behavior
- Verify: test `mg_req_0702_all_errors_reported_together`

### MG-REQ-0703 · No cycles

A cycle between models MUST be an error listing the models of the cycle in order.

- Kind: behavior
- Verify: test `mg_req_0703_cycle_reported_in_order`

### MG-REQ-0704 · Execute follows the plan

Stage 5 MUST execute the approved plan exactly, taking no new decisions. Extraction runs
first, then segments in topological order with independent branches in parallel:

1. MG-REQ-0704.1: a segment starts only when every segment it reads has finished, and at most
   the plan's `max_parallel` segments run at once. A step whose action is `skip` runs nothing.
   Its result is the candidate or the stored result the plan names.

- Kind: behavior
- Verify: test `mg_req_0704_execution_matches_plan`

### MG-REQ-0705 · Candidates are invisible

Results of stage 5 MUST be stored in work as candidates identified by fingerprint, invisible
outside the run:

1. MG-REQ-0705.1: a candidate lives in `.mergus/work/candidates/<fingerprint>/`. It is complete
   only when its last file, `complete.json`, exists. That file is written to a temporary name
   and renamed, and it holds the fingerprint, the run id and the row count. A candidate
   without it is incomplete.
2. MG-REQ-0705.2: "invisible" means that no catalog attaches a candidate and no command lists
   it. Only stages 5 and 6 read candidates.

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
raw load whose fingerprint or `load_id` matches, so that finished work is not repeated:

1. MG-REQ-0707.1: at the start of a run, incomplete candidates are deleted. The fingerprints of
   the complete ones are given to the plan as available results, so a matching step is
   `skip` with reason `result_available`.
2. MG-REQ-0707.2: after the plan is built, a complete candidate that no step of the plan names
   is deleted.

- Kind: behavior
- Verify: test `mg_req_0707_retry_reuses_finished_work`
- Depends: MG-REQ-0143

### MG-REQ-0708 · Cancellation

On cancellation, Mergus MUST stop segments, close readers and connections, keep candidates
useful for resuming, delete other temporary files, and exit with code 7:

1. MG-REQ-0708.1: cancellation is a token that the CLI sets on Ctrl-C (SIGINT, or the console's
   Ctrl-C event on Windows). Stage 5 checks it before each segment and between batches, and it
   kills any child process that is running.
2. MG-REQ-0708.2: complete candidates are kept. Incomplete candidates and the run's scratch
   directory are deleted. The run ends with a `run_finished` event and exit code 7.

- Kind: behavior
- Verify: test `mg_req_0708_cancel_cleans_and_exits_7`

### MG-REQ-0710 · The run and backfill commands

`mergus run [<selection>]` MUST run stages 1 to 5 for the selected models, and `mergus
backfill <model> --from <date> [--to <date>]` MUST do the same with the backfill of
MG-REQ-0657:

1. MG-REQ-0710.1: both take the selection, `--max-wall-time`, `--max-rows-written`,
   `--max-bytes-scanned`, `--idempotency-key` and `--json`, as `plan` does for what they share.
   An error in stages 1 to 4 is reported like `plan`'s and exits with code 1. Nothing is executed
   then.
2. MG-REQ-0710.2: stage 5 writes events as it runs: `plan_ready`, which carries every step's
   action and reason; then, for each computed step, `step_started` and `step_finished`, with
   `step_id` the step's fingerprint, `engine` its segment's engine, `rows` the rows written to
   its candidate and `bytes` the size of the candidate's rows file; then `run_finished`. A
   `skip` step has no step events. The exit code is 0 when every step succeeded or was
   skipped, the exit code of the failing step's error code when a step failed, and 7 when
   cancelled. A step whose engine rejects the user's SQL at execution fails with the next free
   `MG-01xx` code and exit code 1, naming the model and the engine's message.
3. MG-REQ-0710.3: until a phase adds publication (stage 6), a run ends with its results as
   candidates in work. The human output says how many steps were computed and skipped, and that
   nothing was published.
4. MG-REQ-0710.4: Ctrl-C sets the run's cancel token (MG-REQ-0708.1). A second Ctrl-C does not
   skip the finalizer.

- Kind: behavior
- Verify: test `mg_req_0710_run_executes_into_candidates`

### MG-REQ-0709 · Finalizer always runs

Whether a run succeeds or fails, Mergus MUST clean work files not useful for resuming, write
the run report and send notifications:

1. MG-REQ-0709.1: the finalizer runs once at the end of every run: succeeded, failed, cancelled
   or stopped by a limit. It deletes the run's scratch directory and incomplete candidates
   (MG-REQ-0503.1, MG-REQ-0705.1). It never deletes a complete candidate.
2. MG-REQ-0709.2: the run report is the `run_finished` event, with the exit code and, when the
   run failed, its error code. A failure of the finalizer itself never hides the run's own
   error. It is reported as a warning after it.
3. MG-REQ-0709.3: notifications are a no-op until a phase adds them.

- Kind: behavior
- Verify: test `mg_req_0709_finalizer_runs_on_failure`

### MG-REQ-0718 · Run limits

A run MUST accept limits on wall time, rows written and bytes scanned, from `project.toml` or
the command line, the stricter one winning. Before a limit is exceeded the run MUST stop the
step, publish nothing, and end with a typed error naming the limit:

1. MG-REQ-0718.1: the limits are `[limits] wall_time`, `rows_written` and `bytes_scanned` in
   `project.toml`, and the flags `--max-wall-time`, `--max-rows-written` and
   `--max-bytes-scanned`. A wall time is a string of a positive integer and a unit, `s`, `m` or
   `h` (`"90s"`, `"30m"`, `"2h"`); rows and bytes are positive integers (the flags' schema type
   is `string` for the first and `integer` for the others). An invalid value is an invalid
   command line or configuration (MG-REQ-0297). When a file value and a flag value are both
   set, the smaller one applies.
2. MG-REQ-0718.2: bytes scanned are the sum of the sizes of the DuckLake files the plan reads.
   They are checked before stage 5 starts. Rows written are counted per batch, and the run
   stops before it writes the batch that would cross the limit. Wall time is measured from the
   run's start, and checked before each segment and between batches.
3. MG-REQ-0718.3: a limit stop takes the next free `MG-07xx` code and exit code 1. It names the
   limit, its value and the amount reached, and it goes through the finalizer like any failure.

- Kind: behavior
- Verify: test `mg_req_0718_run_stops_at_limit_without_publishing`
- Decision: 0010
