# mergus-adapter-ducklake

DuckLake storage adapter.

## What it does

- Implements the `StorageAdapter` trait of `mergus-adapter` over one DuckLake catalog, through
  the `Database` port of `mergus-ports` only (MG-REQ-0194).
- Declares native versioning, multi-table transactions, the write modes append and replace,
  and the Arrow types it stores exactly.
- `write` stages rows in tables of the connection's in-memory database, outside the catalog:
  readers of the published version never see them (MG-REQ-0286).
- `commit` moves every staged table into the catalog in one transaction and returns the new
  snapshot id; on failure it rolls back and publishes nothing.
- `read_at` reads a table at a snapshot, and fails with `AdapterError::VersionUnavailable`
  when the snapshot has expired or does not exist (MG-REQ-0252).
- `expire` expires the snapshots that are not the newest, not live and not inside the
  rollback window, then deletes their files (MG-REQ-0288).

## What it does not

- It does not implement the replace-period and merge-by-key write modes.
- It does not find live versions itself: until manifests exist, the caller passes them.
- It never depends on the `duckdb` crate.
