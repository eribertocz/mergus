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
period.

- Kind: behavior
- Verify: test `mg_req_0652_only_affected_periods_recomputed`

### MG-REQ-0653 · Incremental by key

An `incremental` model with `key` and no `partition_by` MUST merge new and changed rows by
key, using the adapter's merge mode.

- Kind: behavior
- Verify: test `mg_req_0653_incremental_merge_by_key`

### MG-REQ-0654 · Incremental needs a period or a key

An `incremental` model without `partition_by` and without `key` MUST be a validation error.

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
intermediates live in a private internal zone, never in gold.

- Kind: behavior
- Verify: test `mg_req_0656_intermediates_not_stored_by_default`

### MG-REQ-0657 · Backfill

`mergus backfill <model> --from <date> [--to <date>]` MUST recompute the periods in the range
and publish them atomically with the rest of the run.

- Kind: behavior
- Verify: test `mg_req_0657_backfill_recomputes_range`
