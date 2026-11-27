# mergus-plan architecture

## Main types

| Type | Role |
|---|---|
| `PlanModel` | One model of the read project: name, identifier, code, effective config, inputs, publication, steps |
| `PlanRequest` | What the caller gives: selection, available results, versions, environment, budget |
| `FingerprintInputs` | Every part of a fingerprint, and nothing else |
| `Plan` | The plan: its contract type and its validated document; read-only |
| `Gate` | One gate of stage 4, in the order of the gate table |
| `GateInput` | What the caller gives stage 4: a `GateCheck` per gate, the required secrets, their `Existence` |
| `GateReport`, `GateResult` | Every gate with `Passed`, `Blocked` (its errors) or `NotApplicable` (a later phase) |
| `Cleared`, `Blocked` | The clearance stage 5 needs, or every error of every blocking gate |
| `GateCheck` | `Checked` (the errors found) or `NotApplicable` (the phase waited for) |
| `Existence`, `RequiredSecret` | `Resolves` or `Missing`, and a secret reference a step needs |
| `ModelStep`, `OutputColumn`, `ColumnRead`, `Engine` | One step of a model: its engine, its typed output columns, the columns it reads from earlier steps, its detected operations |
| `Segments`, `Fused`, `Crossing`, `CrossingColumn` | The fused segments of a model, and the typed columns crossing between each pair |
| `Operation`, `Detected` | A non-deterministic operation, and where a step uses it |
| `Refresh`, `By`, `Deletes` | How a model is refreshed: full, incremental by period or by key, or history with its key, `by` and `deletes` |
| `Storage` | Where a result is stored: published, internal (a materialized intermediate) or not stored |
| `Instant`, `Range`, `Grain` | A UTC instant to the second, a half-open range of them, and the grain of a period |
| `Backfill` | The `--from` and `--to` (or run start) of a backfill |
| `RefreshError` | A refresh error a user sees, with its `MG-07xx` code |
| `PlanError` | Refresh errors, or an internal error: an unknown selected model, a cycle, a broken contract, a bad stage-4 input, a bad option value, a bad backfill |

Functions: `plan`, `refresh`, `storage`, `segment`, `type_passport`, `detect_sql`, `determinism`, `fingerprint`, `effective_config`, `lf_text` and `evaluate`.

## Data flow

1. The caller reads the project with `mergus-project` (`read_layout`, `read_model_options`,
   `Identifiers`, `read_sql_references`) and builds one `PlanModel` per model with
   `PlanModel::from_project`; a Python model has no input in 0.2. A `.sql` model gets one
   DuckDB step named like the model (MG-REQ-0631.2).
2. `plan` reads the refresh and storage of every model, in name order, and returns every
   refresh error together (MG-REQ-0654, MG-REQ-0657.2, MG-REQ-0661).
3. `plan` orders every model after the models it reads (depth first, in name order) and
   computes each fingerprint in that order, so an upstream fingerprint is known before it is
   used. A raw input takes the version the caller gives, or `null`.
4. Each selected model gets its depth: the most selected models on a chain of models
   upstream of it. Steps of one depth are cut, in name order, into groups of at most
   `max_parallel`.
5. Every model upstream of a selected model, not selected, whose fingerprint is not
   available, is listed in `stale_upstream` with `+` and the first selected model downstream
   of it in name order.
6. A model with steps gets its `segments`: `segment` starts a new segment at each change of
   engine. A crossing from segment `a` to a later segment `b` holds the columns that steps of
   `b` read from steps of `a`, each once, in the output order of the steps of `a`. The
   builder then checks the conditions of MG-REQ-0747.4 that the schema cannot express.
   Each boundary's transport is `arrow` when `PlanRequest::rows` of the model times the
   widths of its columns is at most 1 GiB, `parquet` otherwise or without an estimate. Each
   Polars segment takes the highest level of this build, `subprocess`.
7. Each step gets its refresh fields. A model incremental by period gets `{mode: period}` and
   its periods: the caller's `PlanRequest::affected` ranges, plus a backfill's range from the
   start of the period holding `from` to the end of the period holding `to`, merged when they
   overlap or touch. A backfilled step is `compute` even when its result is available, and a
   backfilled `full` or `history` model is computed whole. A model by key gets `{mode: key}`,
   and the others get neither field.
8. The document is validated, `plan_id` is the SHA-256 of its canonical JSON without
   `plan_id`, and the document with `plan_id` is validated again. `Plan::bytes` is its
   canonical JSON.

Stage 4: `evaluate` takes, for each gate but credentials, the errors the caller found or the
later phase its checks belong to (`NotApplicable`, which does not block). A gate not given,
an error given under another gate, or a phase that is not a version after `0.4` (compared
number by number) is an internal error. The credentials gate is always checked: every model and step that needs a
reference the existence map marks `Missing` gets an `MG-0701` error, in reference, model and
step order; a reference the map does not answer is an internal error. `GateReport::clear` collects the errors of every blocking gate into `Blocked`, or
returns `Cleared`, which only this crate can build.

The `Type passport` and `Determinism` gates are computed here and given to `evaluate` by the
caller: `type_passport` reads each crossing from a DuckDB segment to a Polars segment, and
`determinism` compares the `Detected` operations of each step of a published model with its
`nondeterministic` option. `detect_sql` walks the `json_serialize_sql` tree: a node with
`modifiers` holding a `LIMIT_MODIFIER` and no `ORDER_MODIFIER` with an order, and every
`FUNCTION` or `WINDOW` node named `any_value`, `random` or `now` in any case. A `LIMIT_MODIFIER`
whose `limit` is `null` (an `OFFSET` alone) is not a limit.

Only `BTreeMap` and `BTreeSet` are used, so no output depends on hash order (MG-REQ-0005).

## Dependencies

- `mergus-contracts`: the generated `mergus_plan` type, `validate`, `canonical_json` and
  `hash::sha256_hex`, the SHA-256 behind a Mergus interface (MG-REQ-0186).
- `mergus-project`: the read project (`ModelOptions`, `Effective`, `Identifier`).
- `serde_json`: the JSON values of options and documents.
- `thiserror`: derives `PlanError` and `Blocked`.
- Dev-dependencies on `mergus-ports` and `mergus-sim`: the tests build the existence map from
  a simulated keychain whose value must never reach an output.
