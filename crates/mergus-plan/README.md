# mergus-plan

Resolve, order and fingerprint the work; produce the plan.

## What it does

- Gives each model a fingerprint: the lowercase hexadecimal SHA-256 of the canonical JSON of
  its code (CRLF turned into LF), its effective option values without origins, each input
  name to the upstream fingerprint or the version the caller gives or `null`, its run
  parameters (`{}` in 0.2), the engine versions the caller gives and its output contract
  (`null` until locked outputs exist) (MG-REQ-0711, MG-REQ-0743).
- Makes one step per selected model: `skip` with `result_available` when its fingerprint is
  among the available results the caller gives, otherwise `compute` with `no_result`; refresh
  `full` (MG-REQ-0712, MG-REQ-0713, MG-REQ-0744.2).
- Reads each model's refresh from its options (MG-REQ-0651 to MG-REQ-0654, MG-REQ-0661):
  - `full` by default: the whole result is rebuilt when the fingerprint changes.
  - `incremental` with `partition_by`, with or without `key`: `step.incremental` is
    `{mode: period}`. `step.periods` holds the affected periods the caller gives, those that
    overlap or touch merged, sorted by `start`, as RFC 3339 UTC timestamps. The list is empty
    when nothing is recomputed. A period is the half-open UTC range of one grain, and weeks
    start on Monday.
  - `incremental` with `key` only: `{mode: key}` and no periods.
  - `incremental` with neither: `MG-0704`.
  - `history` needs `key` and `by` (`MG-0705` otherwise), and `deletes` defaults to `ignore`.
    Its step stays `full`.
  Every model's refresh errors come back together, in model name order.
- Plans a backfill (MG-REQ-0657). The backfilled step is `compute` with `no_result` even when
  its result is available. A model incremental by period adds the backfill range, widened to
  whole periods of its grain, to its affected periods. A `full` model with `partition_by` is
  computed whole, with no periods. A `history` model with `partition_by` needs
  `history_rebuild_confirmed`. Any other case is `MG-0706`: a model with no `partition_by`,
  or a history model whose rebuild is not confirmed.
- Tells, through `Plan::storage` and `Plan::stored`, whether each step's result is stored: a
  published model where it is published, an intermediate with `materialize = true` in the
  internal zone, any other intermediate not at all (MG-REQ-0656).
- Cuts the steps of each depth, in name order, into parallel groups of at most
  `max_parallel` (4 by default), numbered from 1 by depth then order (MG-REQ-0716,
  MG-REQ-0744.3).
- Lists each stale model left out of the selection and upstream of a selected one, with the
  selector `+<model>` of the first selected model downstream of it (MG-REQ-0717,
  MG-REQ-0745).
- Returns the plan as a `mergus_plan` document valid against `schemas/mergus_plan/v1/`,
  identified by `plan_id`, the SHA-256 of its canonical JSON without `plan_id`
  (MG-REQ-0714, MG-REQ-0742, MG-REQ-0743). The same inputs give the same bytes
  (MG-REQ-0005, MG-REQ-0715).
- Evaluates every gate of stage 4 with `evaluate` and returns each gate's result in the
  order of the gate table; stage 5 needs the `Cleared` that `GateReport::clear` refuses,
  with the errors of every blocking gate in one output, when any gate blocks (MG-REQ-0720,
  MG-REQ-0722). A gate whose checks belong to a later phase reports `not_applicable` with
  that phase and does not block.
- The credentials gate reads, for each required secret reference, whether it resolves or is
  missing, and reports each missing one as `MG-0701`; it never sees a value (MG-REQ-0721).

- Fuses consecutive steps of one engine of a model into one segment, numbered from 0 in
  execution order, and writes them as the step's `segments` (MG-REQ-0631, MG-REQ-0747.1). A
  `.sql` model is one DuckDB step named like the model, so one segment (MG-REQ-0631.2); a
  Python model gets its steps from the caller.
- Writes, for each pair of segments, a boundary with the columns the later one reads from the
  earlier one, and only those, in their output order (MG-REQ-0632). Columns are told apart
  by step and name, so the passport and the transport width count each one. It crosses as `arrow` when
  the estimated rows the caller gives times the width of its columns is at most 1 GiB, as
  `parquet` above that or with no estimate; `VARCHAR`, `BLOB`, nested, `ENUM` and unknown
  types count 64 bytes (MG-REQ-0633).
- Runs every Polars segment at `subprocess` with reason `not_available`: `native` and
  `in_process` are not in this build (MG-REQ-0635).
- Refuses, as an internal error, a step whose non-Polars segment has a Polars level or whose
  boundaries are not sorted by `from`, then `to` (MG-REQ-0747.4).
- `type_passport` returns an `MG-0702` error of the `Type passport` gate for each column
  crossing from DuckDB to Polars with a lossy type: `HUGEINT`, `UHUGEINT`, `UNION`, `BIT`,
  `INTERVAL`, a `DECIMAL` of more than 38 digits, an unknown type, or a list, struct or map
  holding one (MG-REQ-0634).
- `detect_sql` finds, on the tree of the `SqlParser` port, each `LIMIT` with no `ORDER BY` in
  the same `SELECT` (an `OFFSET` alone is none) and each call of `any_value`, `random` or
  `now`, window calls included; `determinism` returns an
  `MG-0703` error of the `Determinism` gate for each one a published model does not declare
  in `nondeterministic` (MG-REQ-0637). The caller gives both gates' errors to `evaluate`.

## What it does not

- It performs no I/O: the caller reads the project with `mergus-project` and passes the
  models (MG-REQ-0004).
- It executes nothing, stores nothing and writes no file; `mergus plan` (MG-REQ-0746) does.
- It evaluates no gate other than credentials by itself: the caller gives the errors of the gates it
  checked, or the phase they wait for, and whether each required secret reference resolves,
  found by the existence check of `mergus-security`.
- It parses no selector: the caller gives the selected model names.
- It does not read Polars code: the Polars half of the non-determinism detector reads the
  tree of the `Python` port, and comes with Python models. It plans no write strategies or
  publication contents.
