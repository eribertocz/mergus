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
| `pin_recipe` | For a version reference, describes how to read it pinned; stored in the manifest |
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
