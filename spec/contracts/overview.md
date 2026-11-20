---
id: contracts-overview
title: Public contracts, ownership and evolution
crate: mergus-contracts
phase: "0.1"
status: draft
depends_on: [kernel]
---

# Contracts

A contract is a versioned document that crosses a module boundary (MG-REQ-0003). Its JSON
Schema lives in `schemas/`; its Rust types are generated into `mergus-contracts`.

| Contract | Owner | Mergus role | Consumers | File |
|---|---|---|---|---|
| `catalog_schema`, `catalog_state` | Menura | Producer | Menura, via host | [menura-catalog](menura-catalog.md) |
| `producer_requirements`, `integrity_checks`, validation report | Menura | Consumer | Mergus | [menura-inputs](menura-inputs.md) |
| Publication manifest and pointer | Mergus | Owner, producer | Host, executor | [manifest](manifest.md) |
| `--json` events and exit codes | Mergus | Owner, producer | CI, orchestrators, AI agents | [events](events.md) |
| Ops tables | Mergus | Owner, producer | Users, BI, Menura | [ops-tables](ops-tables.md) |
| Storage adapter | Mergus | Owner | Adapter authors | [adapter](adapter.md) |
| `project.toml`, folder `mergus.toml`, `sources/*.toml` | Mergus | Owner, consumer | Users, editors | `project/` area |
| OpenLineage, OpenTelemetry | Open standards | Producer | Lineage and observability tools | `observability/` area |

### MG-REQ-0201 · Schema per contract

Every contract Mergus owns MUST have a JSON Schema 2020-12 file in `schemas/`, with closed
objects (`additionalProperties: false`), at least one valid and one invalid example, and the
fields `contract` and `contract_version` in every document.

- Kind: behavior
- Verify: test `mg_req_0201_every_contract_has_schema_and_examples`

### MG-REQ-0202 · Generated types

Rust types of every contract MUST be generated from its schema into `mergus-contracts`.
Hand-written duplicates of contract types MUST NOT exist.

- Kind: process
- Verify: check `contracts-generated`

### MG-REQ-0203 · Validate before emitting

Every contract document Mergus emits MUST be validated against its schema before it is
written or printed. A document that fails validation MUST NOT be emitted and the run ends
with an internal error.

- Kind: behavior
- Verify: test `mg_req_0203_invalid_document_never_emitted`

### MG-REQ-0204 · Semantic versioning per contract

Each contract has its own semantic version, independent of the Mergus version. Adding an
optional field is a minor change; removing or renaming a field, changing a type or tightening
a constraint is a major change.

- Kind: process
- Verify: check `contract-compatibility`

### MG-REQ-0205 · Compatibility check in CI

CI MUST compare every changed schema with its version on `main` and fail when an
incompatible change does not bump the major version.

- Kind: process
- Verify: check `contract-compatibility`

### MG-REQ-0206 · Readers accept older minor versions

When Mergus reads a document of a contract it owns, it MUST accept every minor version of
the same major version up to the latest it knows.

- Kind: behavior
- Verify: test `mg_req_0206_older_minor_versions_accepted`

### MG-REQ-0207 · External names in one constant

Every name Mergus uses from another module (contract names, extension namespace, schema
`$id`, binary name) MUST be defined once, as a constant in `mergus-contracts`.

- Kind: process
- Verify: review `external-names-constant`

### MG-REQ-0208 · Canonical JSON for hashed documents

Documents that are hashed or signed MUST be serialized as canonical JSON (RFC 8785) before
hashing.

- Kind: behavior
- Verify: test `mg_req_0208_hash_uses_canonical_json`
