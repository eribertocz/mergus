# mergus-contracts architecture

## Main types

| Type | Role |
|---|---|
| `Contract` | Trait of every generated contract type: `NAME`, `VERSION`, `SCHEMA` |
| `ContractSchema` | One entry of the registry `generated::CONTRACTS` |
| `Schema` | A parsed JSON Schema, reduced to the supported keywords (`KEYWORDS`) |
| `SchemaError` | Why a schema cannot be used, with its JSON pointer |
| `Violation` | One way a document breaks its schema, with its JSON pointer |
| `ValidDocument` | A document that passed validation; the only emittable form |
| `EmitError` | Why a document cannot be emitted; always an internal error |
| `names::*` | Constants for every external name (MG-REQ-0207) |

## Data flow

1. A developer edits a schema under `schemas/` and runs `cargo xtask contracts-generate`,
   which writes `src/generated/mod.rs` and one `<contract>_v<version>.rs` per schema.
   `cargo xtask contracts-generated` fails in `just check` when they are out of date.
2. A producing crate builds a generated value and calls `validate`, which serializes it,
   parses the embedded schema and validates the JSON value.
3. On success it gets a `ValidDocument` and writes it through a port. On failure it gets an
   `EmitError` listing every `Violation`, emits nothing, and ends the run with an internal
   error.

The tests compile the types generated from `tests/fixtures/schemas/` into
`tests/fixtures/generated/`, so the generated code itself is built and checked.

## Dependencies

- `serde` and `serde_json`: the generated types derive `Serialize` and `Deserialize`, and
  documents are validated as `serde_json::Value`.
- `thiserror`: derives `SchemaError` and `EmitError`.
- A dev-dependency on `mergus-ports` with the feature `real`: the tests read the schemas and
  their examples through `RealFileSystem`. Dev-dependencies are not linked into other crates.
