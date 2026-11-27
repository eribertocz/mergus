# mergus-plan architecture

## Main types

| Type | Role |
|---|---|
| `PlanModel` | One model of the read project: name, identifier, code, effective config, inputs |
| `PlanRequest` | What the caller gives: selection, available results, versions, environment, budget |
| `FingerprintInputs` | Every part of a fingerprint, and nothing else |
| `Plan` | The plan: its contract type and its validated document; read-only |
| `Gate` | One gate of stage 4, in the order of the gate table |
| `GateInput` | What the caller gives stage 4: a `GateCheck` per gate, the required secrets, their `Existence` |
| `GateReport`, `GateResult` | Every gate with `Passed`, `Blocked` (its errors) or `NotApplicable` (a later phase) |
| `Cleared`, `Blocked` | The clearance stage 5 needs, or every error of every blocking gate |
| `GateCheck` | `Checked` (the errors found) or `NotApplicable` (the phase waited for) |
| `Existence`, `RequiredSecret` | `Resolves` or `Missing`, and a secret reference a step needs |
| `PlanError` | An internal error: an unknown selected model, a cycle, a broken contract, a gate or an existence not given |

Functions: `plan`, `fingerprint`, `effective_config`, `lf_text` and `evaluate`.

## Data flow

1. The caller reads the project with `mergus-project` (`read_layout`, `read_model_options`,
   `Identifiers`, `read_sql_references`) and builds one `PlanModel` per model with
   `PlanModel::from_project`; a Python model has no input in 0.2.
2. `plan` orders every model after the models it reads (depth first, in name order) and
   computes each fingerprint in that order, so an upstream fingerprint is known before it is
   used. A raw input takes the version the caller gives, or `null`.
3. Each selected model gets its depth: the most selected models on a chain of models
   upstream of it. Steps of one depth are cut, in name order, into groups of at most
   `max_parallel`.
4. Every model upstream of a selected model, not selected, whose fingerprint is not
   available, is listed in `stale_upstream` with `+` and the first selected model downstream
   of it in name order.
5. The document is validated, `plan_id` is the SHA-256 of its canonical JSON without
   `plan_id`, and the document with `plan_id` is validated again. `Plan::bytes` is its
   canonical JSON.

Stage 4: `evaluate` takes, for each gate but credentials, the errors the caller found or the
later phase its checks belong to (`NotApplicable`, which does not block); a gate not given is
an internal error. The credentials gate is always checked: every model and step that needs a
reference the existence map marks `Missing` gets an `MG-0701` error, in reference, model and
step order; a reference the map does not answer is an internal error. `GateReport::clear` collects the errors of every blocking gate into `Blocked`, or
returns `Cleared`, which only this crate can build.

Only `BTreeMap` and `BTreeSet` are used, so no output depends on hash order (MG-REQ-0005).

## Dependencies

- `mergus-contracts`: the generated `mergus_plan` type, `validate`, `canonical_json` and
  `hash::sha256_hex`, the SHA-256 behind a Mergus interface (MG-REQ-0186).
- `mergus-project`: the read project (`ModelOptions`, `Effective`, `Identifier`).
- `serde_json`: the JSON values of options and documents.
- `thiserror`: derives `PlanError` and `Blocked`.
