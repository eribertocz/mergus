# mergus-contracts architecture

## Main types

| Type | Role |
|---|---|
| `Contract` | Trait of every generated contract type: `NAME`, `VERSION`, `SEMANTIC_VERSION`, `SCHEMA` |
| `ContractSchema` | One entry of the registry `generated::CONTRACTS` |
| `Schema` | A parsed JSON Schema, reduced to the supported keywords (`KEYWORDS`) |
| `SchemaError` | Why a schema cannot be used, with its JSON pointer |
| `Violation` | One way a document breaks its schema, with its JSON pointer |
| `ValidDocument` | A document that passed validation; the only emittable form |
| `EmitError` | Why a document cannot be emitted; always an internal error |
| `ReadError` | Why a document cannot be read as a contract, with every `Violation` |
| `validate_event`, `required_fields` | Validate a `mergus_events` event and check the fields its type requires (MG-REQ-0295) |
| `generated::mergus_events_v1` | The events of `--json` output |
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

4. A consuming crate passes a JSON value to `read`, which validates it against the latest
   schema of the major version and deserializes it. A minor version only adds optional
   fields, and the `contract-compatibility` guard refuses any other in-place change of a
   schema, so documents of older minor versions pass (MG-REQ-0206).
5. Before hashing or signing a document, a crate takes `ValidDocument::hash_input`, or
   `canonical_json` of a value it read: RFC 8785 canonical JSON, written in-house
   (MG-REQ-0185, MG-REQ-0208). Members are sorted by UTF-16 code units, and numbers are
   written as ECMAScript writes doubles, from the shortest digits that read back.

The tests compile the types generated from `tests/fixtures/schemas/` into
`tests/fixtures/generated/`, so the generated code itself is built and checked.

## Dependencies

- `serde` and `serde_json`: the generated types derive `Serialize` and `Deserialize`, and
  documents are validated as `serde_json::Value`. The feature `float_roundtrip` reads every
  number as its closest double, which canonical JSON needs.
- `thiserror`: derives `SchemaError` and `EmitError`.
- `sha2` without default features: the SHA-256 of `hash` (RustCrypto), so no other crate
  depends on it (MG-REQ-0186).
- A dev-dependency on `mergus-ports` with the feature `real`: the tests read the schemas and
  their examples through `RealFileSystem`. Dev-dependencies are not linked into other crates.
