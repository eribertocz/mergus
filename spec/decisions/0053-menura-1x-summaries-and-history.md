# 0053 · Menura 1.x: summaries come from integrity checks; history validity is a model concern

- Status: accepted
- Context for: phase 0.5 (`contracts/menura-inputs.md`, MG-REQ-0237), MG-REQ-0663

## Context
Menura is at 1.1.1 with contracts frozen at major version 1. Mergus's specification still names
Menura 0.x versions and a `producer_requirements` document, which Menura never shipped. The
owner relayed the answers of Menura's specifier.

## Decision
1. **Summaries.** Mergus takes the summaries to build from `integrity_checks(handle, dialect)`,
   which returns `menura.integrity_checks`. The document carries one check of kind `summary` for
   each summary that the semantic model declares (Menura `summaries` extension).
   - Each check's `build = {sql, parameters, columns}` is the only allowed way to build that
     summary (Menura CHK-5.1, CHK-5.4). Its result has exactly `build.columns`, in that order,
     with one row per group (CHK-5.5).
   - A parameter has a fixed `value` or a `binding`. The bindings are `time_zone`, `period_from`
     and `period_to` (CHK-7.1–7.4).
     - Mergus chooses `time_zone`, and binds the same value in the build and in its check.
     - The periods are canonical periods at the grain of the base table's partitions, as an
       inclusive range covering whole periods.
   - After a build, Mergus publishes `catalog_state.aggregate_build`:
     - `definition_hash`, copied from the check;
     - `coverage`;
     - `built_from`, every base period used;
     - `time_zone`;
     - `integrity_check`, the result of the check on the `verified` versions, and only when that
       check finished (CAT-7.1).
   - A summary without a finished check of its current build is left out of the state.
   - `compile` is never used on a summary.
   - `producer_requirements` is dropped: `integrity_checks` replaces it.
2. **History tables.** `catalog_schema` carries no temporal meaning, and Mergus adds none to it.
   - `_valid_from`, `_valid_to`, `_is_current` and `_deleted` are ordinary columns there.
   - Temporal meaning belongs to the semantic model, through Menura's `populations` extension in
     `validity` mode.
   - That mode expects an inclusive end, with NULL meaning open. Mergus keeps the half-open range
     with the `9999-12-31` sentinel (MG-REQ-0663), which is exact for timestamps.
   - Mergus asks Menura, by a `consumer-request` issue, for native support of a half-open end
     with a sentinel, and for "as of" joins of facts against SCD2 dimensions. Until then, a
     model maps the end through a derived field: the inclusive end, or NULL when the value is the
     sentinel.

## Consequences
- The phase 0.5 issues rewrite `contracts/menura-inputs.md` and MG-REQ-0237 around
  `integrity_checks`.
- They also update the roadmap's external dependencies to Menura 1.x.
