# mergus-runtime architecture

## Main types

| Item | Role |
|---|---|
| `zone_storage` | The `ZoneStorage` of each zone of a `Configuration` |
| `ZoneStorage` | A zone's profile, location, catalog, data path and clients |
| `Zone` | `raw`, `gold` or `ops` |
| `Profile` | `local`, `shared` or `cloud` |
| `Catalog` | A DuckDB file, a SQLite file or a PostgreSQL secret; its attach string and extension |
| `Clients` | One process, several local processes, or several remote clients |
| `check_storage` | The `StorageProblem`s of raw and gold (MG-REQ-0501) |
| `StorageProblem` | One problem: code, file, line, key and message |
| `open_raw` | The DuckLake adapter of raw, attached encrypted (MG-REQ-0515) |
| `RawLoad` | The rows one load extracted from one table, and the keys it found deleted |
| `write_raw` | Writes a `RawLoad` into `__changes` and the current table in one commit |
| `RawError` | Why raw could not be opened or written, with its code |

## Data flow

1. The caller reads the project with `mergus_project::read_configuration`; only a valid
   configuration is used, so each zone has a known profile, a location and, with `cloud`, a
   catalog secret reference.
2. `zone_storage` takes raw, gold and ops in that order and fills in the defaults of the
   profile, then the overrides `catalog` and `data_path`.
3. `check_storage` compares raw and gold. Locations are compared by their parts between `/`
   or `\`, ignoring empty and `.` parts, so `lake` and `./lake/` are the same location and
   `lake/raw` lies inside `lake` but `lake/rawgold` does not. Locations are compared when both
   zones are `local` or `shared`, or both `cloud`; catalog secrets when both are `cloud`.
4. It reads `project.toml` again through the `FileSystem` port only to find the line of the
   key of gold with `mergus_project::key_line`.

## Writing raw

1. `open_raw` checks a `local` or `shared` catalog file against the project root. The check
   is on the paths' parts, without touching the disk: `.` parts are dropped and each `..`
   removes the part before it. The root is absolute. It then opens `DuckLakeAdapter` on the
   `Database` through a wrapper whose connections turn `attach_ducklake` into
   `attach_ducklake_encrypted`. The adapter never attaches raw unencrypted, and it does not
   change for this. The port's refusal of an unencrypted catalog becomes `MG-0503`, naming
   the catalog by its file or its secret reference, never by its connection value.
2. `write_raw` adds the metadata columns to the load's rows. For a table with `key` it then:
   - checks that no key repeats, comparing keys in Arrow's row format;
   - reads the current table at the version of the table's last commit;
   - keeps the rows whose key the load neither returned nor found deleted;
   - turns the current rows found deleted into delete rows with the load's metadata.
   It appends the load's rows and the delete rows to `__changes`, replaces the current table,
   and commits. Rows read back are cast to the raw schema by column name.

## Dependencies

- `arrow`: the record batches of raw, their keys and filters; the version of
  `mergus-adapter`.
- `mergus-adapter`: the `StorageAdapter` contract raw is written through.
- `mergus-adapter-ducklake`: the DuckLake adapter raw is stored in.
- `mergus-ports`: the `FileSystem`, `Database` and `Clock` ports, and `PortError`.
- `mergus-source`: `DeclaredTable` and the raw table names (MG-REQ-0403).
- `mergus-project`: `Configuration`, and `key_line`, so that this crate parses no TOML
  (MG-REQ-0186).
- `serde_json`: the JSON values of `project.toml`.
- `thiserror`: derives `StorageProblem`.
- `mergus-sim`, as a dev-dependency only: the in-memory file system and the simulated
  `Database` of the tests. `mergus-ports` with `real`, as a dev-dependency only: the real
  DuckDB of the encryption test.
