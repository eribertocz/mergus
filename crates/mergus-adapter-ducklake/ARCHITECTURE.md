# mergus-adapter-ducklake architecture

## Main types

| Type | Role |
|---|---|
| `DuckLakeAdapter` | The `StorageAdapter` over one attached DuckLake catalog |
| `DuckLakeError` | Why the storage failed, carried by `AdapterError::Storage` |

## Data flow

1. `DuckLakeAdapter::open` opens an in-memory connection of the `Database` port and
   attaches the catalog.
2. `write` creates a staging table `mergus_stage_<n>` in the in-memory database and appends
   the Arrow batches to it. Nothing in the catalog changes.
3. `commit` runs one transaction: for each staged write, it creates the catalog table when
   missing, empties it for replace, and inserts the staged rows by column name. DuckLake
   publishes the transaction as one snapshot, whose id is the `VersionRef`. The staging
   tables are then dropped.
4. `pin_recipe` checks that the snapshot exists, then returns the canonical JSON of
   `format` (`ducklake`), `table` and `version` (the snapshot id in decimal). It holds no
   catalog location.
5. `read_at` checks that the snapshot exists, then reads the table `AT (VERSION => id)`.
6. `expire` lists the snapshots and their times, keeps the newest, the live ones and those
   younger than the rollback window by the `Clock` port, and expires the rest.

## Dependencies

- `mergus-adapter`: the trait and its types.
- `mergus-ports`: the `Database` and `Clock` ports.
- `mergus-contracts`: `canonical_json` for the pin recipe.
- `serde_json`: the pin recipe object.
- `arrow`, without default features: record batches and data types.
- `thiserror`: derives `DuckLakeError`.
