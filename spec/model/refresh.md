---
id: model-refresh
title: Refresh modes, periods, late data and materialization
crate: mergus-plan
phase: "0.4"
status: draft
depends_on: [project-folder-config]
---

# Refresh

| `refresh` | Behavior |
|---|---|
| `full` (default) | Rebuild the whole table |
| `incremental` | Recompute only affected periods (`partition_by`) or merge by `key` |
| `history` | Keep every version of each key (`model/history.md`) |

### MG-REQ-0651 · Full by default

A model without `refresh` MUST be rebuilt completely when its fingerprint changes.

- Kind: behavior
- Verify: test `mg_req_0651_full_refresh_default`

### MG-REQ-0652 · Incremental by period

An `incremental` model with `partition_by` MUST recompute only the periods whose input rows
changed. A change in an input without periods (for example a dimension) MUST recompute every
period:

1. MG-REQ-0652.1: a period is the half-open UTC range `[start, end)` of one grain of the
   `partition_by` column. `week` starts on Monday (ISO 8601). A `DATE` value is read as
   midnight UTC.
2. MG-REQ-0652.2: the plan touches no data (MG-REQ-0701). The caller gives the affected periods
   of each such model:
   - the periods of the partition values of the input rows read since the last run, for an
     input that has the model's partition column;
   - every period, for an input that does not, given as one range from the first period's
     start to the last period's end.
3. MG-REQ-0652.3: the plan writes `step.incremental` `{mode: period}` and the periods it
   recomputes in `step.periods`. Periods that overlap or touch are merged, and the list is
   sorted by `start`. An empty list means nothing is recomputed.

- Kind: behavior
- Verify: test `mg_req_0652_only_affected_periods_recomputed`

### MG-REQ-0653 · Incremental by key

An `incremental` model with `key` and no `partition_by` MUST merge new and changed rows by
key, using the adapter's merge mode:

1. MG-REQ-0653.1: the plan writes `step.incremental` `{mode: key}` and no `step.periods`.

- Kind: behavior
- Verify: test `mg_req_0653_incremental_merge_by_key`

### MG-REQ-0654 · Incremental needs a period or a key

An `incremental` model without `partition_by` and without `key` MUST be a validation error:

1. MG-REQ-0654.1: the error is raised when the plan is built, with the next free `MG-07xx` code
   and exit code 1. With both `partition_by` and `key`, the model is by period.

- Kind: behavior
- Verify: test `mg_req_0654_incremental_needs_partition_or_key`

### MG-REQ-0655 · Lookback

With `lookback = n`, the last `n` periods MUST be recomputed on every run, in addition to the
affected ones. A recomputed period whose content did not change keeps its token.

- Kind: behavior
- Verify: test `mg_req_0655_lookback_recomputes_without_token_noise`
- Phase: 0.5
- Depends: MG-REQ-0213

### MG-REQ-0656 · Intermediates are computed on the fly

Intermediate models MUST NOT be stored unless `materialize = true`; materialized
intermediates live in a private internal zone, never in gold:

1. MG-REQ-0656.1: the plan exposes, in its Rust API, which models produce stored output. The
   plan contract carries no field for it in 1.1.0.

- Kind: behavior
- Verify: test `mg_req_0656_intermediates_not_stored_by_default`

### MG-REQ-0657 · Backfill

`mergus backfill <model> --from <date> [--to <date>]` MUST recompute the periods in the range
and publish them atomically with the rest of the run:

1. MG-REQ-0657.1: the range is widened to whole periods of the model's grain: from the start of
   the period holding `--from`, to the end of the period holding `--to`. Without `--to`, it
   ends at the end of the period holding the run's start.
2. MG-REQ-0657.2: the plan writes those periods as for MG-REQ-0652.3, whatever the changes.
   A model with no `partition_by` cannot be backfilled, which is a typed `MG-07xx` error.
3. MG-REQ-0657.3: a backfilled model's available result does not count, so its step is
   `compute` with reason `no_result`. Its periods are the backfill range merged with the
   affected periods the caller gives.
4. MG-REQ-0657.4: a `full` model with `partition_by` is backfilled whole: its step is `compute`
   and has no `periods`. A `history` model cannot be backfilled unless the request carries the
   rebuild confirmation of MG-REQ-0665; without it the backfill is the same typed error.

- Kind: behavior
- Verify: test `mg_req_0657_backfill_recomputes_range`
