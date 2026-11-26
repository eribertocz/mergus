---
id: contracts-adapter
title: The storage adapter contract
crate: mergus-adapter
phase: "0.3"
status: draft
depends_on: [contracts-overview]
---

# Storage adapter

An adapter connects Mergus to one storage format. The rest of Mergus does not know which
storage is behind it. The contract is a public Rust trait in `mergus-adapter`, protected by
`cargo-semver-checks`.

| Operation | Does |
|---|---|
| `capabilities` | Declares: versioning (native, by files, none), multi-table transactions, concurrent clients, exactly stored types, write modes |
| `write` | Receives an Arrow stream for one table and a mode: append, replace, replace period, merge by key |
| `commit` | Commits everything written and returns the data version reference |
| `pin_recipe` | For a table and a version reference, describes how to read that table pinned at that version (MG-REQ-0298); stored in the manifest |
| `read_at` | Reads a table at a version, for checks, summaries and comparisons |
| `expire` | Deletes old versions, never one referenced by a live manifest or inside the rollback window |

| Storage | Phase |
|---|---|
| DuckLake | MVP |
| Iceberg, Delta, ClickHouse | After the MVP |

### MG-REQ-0285 · Six operations

Every adapter MUST implement exactly the six operations of this table through the
`StorageAdapter` trait.

- Kind: behavior
- Verify: test `mg_req_0285_adapter_trait_has_six_operations`

### MG-REQ-0286 · Nothing visible before commit

Data written with `write` MUST NOT be visible to readers of the published version before
`commit` returns.

- Kind: behavior
- Verify: test `mg_req_0286_write_invisible_before_commit`

### MG-REQ-0298 · The pin recipe is structured

`pin_recipe` MUST return, as the canonical JSON (MG-REQ-0208) of one object, exactly these
keys:

1. MG-REQ-0298.1: `format`: the storage format, `ducklake` in the MVP;
2. MG-REQ-0298.2: `table`: the physical table, as the store names it;
3. MG-REQ-0298.3: `version`: the native version as a string; for DuckLake, the snapshot id in
   decimal.

The recipe MUST NOT hold a catalog location, a credential or a secret value: where a table's
store is and how to reach it is given by the manifest entry (phase 0.5), never by the recipe.
A reader reads `table` at `version` with the format's own pinned read; for DuckLake, the
catalog attached read-only at that snapshot.

- Kind: behavior
- Verify: test `mg_req_0298_pin_recipe_is_structured`
- Phase: 0.3
- Decision: 0043

### MG-REQ-0287 · Honest capabilities

An adapter MUST declare only capabilities it implements. Mergus MUST refuse a write mode or a
feature the adapter does not declare, with exit code 1, before extracting any data.

- Kind: behavior
- Verify: test `mg_req_0287_undeclared_capability_refused_early`

### MG-REQ-0288 · Expire never removes what is in use

`expire` MUST NOT delete a version referenced by any live manifest or inside the rollback
window.

- Kind: behavior
- Verify: test `mg_req_0288_expire_keeps_referenced_versions`

### MG-REQ-0289 · Conformance kit

`mergus-certify` MUST provide a test suite that every adapter passes before it is accepted;
it covers the six operations, crash points between write and commit, and concurrency.

- Kind: behavior
- Verify: test `mg_req_0289_certify_kit_runs_on_ducklake`
