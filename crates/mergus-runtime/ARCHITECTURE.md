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

## Dependencies

- `mergus-ports`: the `FileSystem` port and `PortError`.
- `mergus-project`: `Configuration`, and `key_line`, so that this crate parses no TOML
  (MG-REQ-0186).
- `serde_json`: the JSON values of `project.toml`.
- `thiserror`: derives `StorageProblem`.
- `mergus-sim`, as a dev-dependency only: the in-memory file system of the tests.
