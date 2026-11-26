# 0042 · Storage profile inputs, and zone rules moved to their phases

- Status: accepted
- Date: 2026-11-26
- Answers: #202

## Context

#202 showed that the zone and profile rules of phase 0.3 named inputs that nothing defined:
credentials of a store, `temp_dir` and `temp_max`, the work zone, the executor, profile
defaults and concurrency.

Verified facts (https://ducklake.select/docs/stable/duckdb/usage/choosing_a_catalog_database,
`.../duckdb/usage/connecting`, DuckLake 1.1):

- Catalogs attach as `ducklake:<file>` (DuckDB), `ducklake:sqlite:<file>` (needs `sqlite`)
  and `ducklake:postgres:<conninfo>` (needs `postgres`, PostgreSQL 12 or newer).
- A DuckDB catalog: "only one process can attach it at a time unless every process attaches
  it with `READ_ONLY`". SQLite is for "multiple local clients"; PostgreSQL for "a multi-user
  lakehouse with potentially remote clients".
- `DATA_PATH` defaults to "`metadata_file.files` for DuckDB files, required otherwise", and is
  stored in the catalog.

## Decision

- Phase 0.3 keeps MG-REQ-0501 and MG-REQ-0521, now concrete: each profile derives catalog and
  data path from `location`; `cloud` also needs the secret reference `catalog`; `catalog` and
  `data_path` may be overridden. Raw and gold must not share a location (local, shared) or a
  catalog reference and location prefix (cloud).
- MG-REQ-0503 (work zone) moves to phase 0.4, when engines write it. MG-REQ-0504 (spill) moves
  to phase 0.9 with `[resources]`, where `temp_dir` and `temp_max` are defined.
- MG-REQ-0522 to MG-REQ-0525 (guarantee levels, executor, concurrent readers, never a wrong
  number) move to phase 0.5, with publication and the executor.
- MG-REQ-0194 may install `sqlite` and `postgres` for DuckLake catalogs.

## Consequences

- #197 shrinks to MG-REQ-0501 and MG-REQ-0521 and also changes the `project.toml` schema in
  `mergus-project`.
- Phase 0.3 has six rules fewer; phases 0.4, 0.5 and 0.9 gain them.
