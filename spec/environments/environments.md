---
id: environments-environments
title: Environments as pointers, logical and physical tables
crate: mergus-runtime
phase: "0.7"
status: draft
depends_on: [contracts-manifest]
---

# Environments

An environment is only a pointer to a manifest. Data versions never change, so several
environments can point to the same data without copying it.

| Environment | Purpose | Writes to | Reads |
|---|---|---|---|
| `dev-<person>` | Iterate | Its own isolated store; its credential can never write prod | Prod gold read-only; a coherent raw sample |
| `staging`, `ci-<n>` | Validate a change on real data | New physical tables in the prod store, invisible, with the prod identity | Prod |
| `prod` | What dashboards see | Gold | — |

```toml
[environments.dev]
gold = { profile = "local", location = "./.dev/gold" }
[environments.prod]
gold = { profile = "cloud", location = "s3://my-company/gold" }
```

### MG-REQ-1201 · Logical and physical tables

A published model MUST have a stable logical name and one physical table per definition
version (hash of canonical code and configuration, without data versions), for example
`fact_sales__a3f9`. The manifest maps logical name → physical table → data version.

- Kind: behavior
- Verify: test `mg_req_1201_physical_table_per_definition`

### MG-REQ-1202 · Never write another definition's table

A run MUST NOT write a physical table of a different definition. When a model is unchanged,
environments share its physical table, but only `prod` writes to it; others read it pinned.

- Kind: behavior
- Verify: test `mg_req_1202_only_prod_writes_shared_tables`

### MG-REQ-1203 · Reuse from prod

An environment MUST build only models whose definition changed and their affected
dependents; everything else MUST be read from prod pinned to its published version.

- Kind: behavior
- Verify: test `mg_req_1203_unchanged_models_read_from_prod`

### MG-REQ-1204 · Change classification

Each change MUST be classified as form only (nothing rebuilt), compatible (only that model),
or breaking (that model and its dependents), by analysing the SQL and the Polars plan.

- Kind: behavior
- Verify: test `mg_req_1204_change_classification`

### MG-REQ-1205 · Coherent dev sample

A `dev-*` environment MUST receive raw as a deterministic sample over a business key
(default 1% of keys) applied coherently across tables, with pseudonyms computed with the dev
key and `allow` PII masked.

- Kind: behavior
- Verify: test `mg_req_1205_dev_sample_coherent_and_masked`

### MG-REQ-1206 · Per-environment configuration

`[environments.<name>]` MAY override only gold location, credentials and source bindings.
`--env <name>` selects the environment for any command.

- Kind: behavior
- Verify: test `mg_req_1206_environment_overrides`

### MG-REQ-1207 · Cleanup by references

A physical table and its versions MUST be kept while any manifest of any environment uses
them or they are inside the rollback window. Temporary environments MUST be deleted when
their pull request closes or after a configurable idle time.

- Kind: behavior
- Verify: test `mg_req_1207_cleanup_by_reference`
