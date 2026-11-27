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
| `load_raw` | Writes a load with `write_raw`, then records it in `ops.loads` |
| `load_raw_off`, `load_raw_off_failed` | Record the extraction of a source with `raw = false`, done or failed |
| `Watermarks` | The watermarks before and after a load, in text |
| `record_load` | The one writer of `ops.loads`, for `mergus-ops` to take over |
| `LoadRecord`, `LoadMode`, `LoadResult` | A row of `ops.loads`, its `mode` and its `result` |
| `new_load_id` | A UUID version 7 from the `Clock` and `Random` ports |
| `last_watermark` | The last recorded `watermark_after` of a table |
| `source_retention`, `raw_retention` | The retention of a source, and of the raw catalog |
| `expire_raw` | Expires raw with `raw_retention`; maintenance only |
| `raw_off_sources`, `keeps_raw` | The sources with `raw = false` |
| `RawError` | Why raw could not be opened or written, with its code |
| `load_table`, `TableLoad`, `Stores`, `LoadOutcome` | An incremental load by chunks, resumed when interrupted |
| `LoadError` | Why `load_table` failed, with its code |
| `table_loads`, `TableLoads`, `LastLoad` | The last load of a table and the ids of its recorded loads |
| `finished_changes` | The rows of `__changes` of recorded loads, those readers see |

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

## The load registry

1. `load_raw` takes the time of the end of the raw commit from the `Clock`, so `duration_ms`
   runs from the load's `_extracted_at` to it. Its `mode` is `incremental` when the table has
   `incremental` and the load starts from a watermark, `full` otherwise.
2. The registry row is written and committed through the ops adapter after the raw commit,
   since `[ops]` may be another store (MG-REQ-0514.2). When the raw write fails, the row is
   still written, with the error's code, and the raw error is returned. When the row cannot
   be written, `RawError::Registry` is returned: the raw version stays, unrecorded, and is
   ignored.
3. `last_watermark` reads `loads` at a version of the ops store and keeps the
   `watermark_after` of the table's row with the greatest `started_at`, then the greatest
   `load_id`, since storage returns rows in no set order. Failed rows count, as their
   `watermark_after` equals their `watermark_before`.
4. `load_raw_off_failed` records a failed extraction of a source with `raw = false`, with the
   code the caller gives.

## Loads by chunks

1. `load_table` reads `ops.loads` for the table's last row, its watermark and
   `deletes_checked_at`, and the ids of its recorded loads. It reads `__changes` at the latest
   raw version: the latest load id there without a row, by `_extracted_at`, is unfinished.
   For a table with `key` it is resumed: same load id, same `_extracted_at`, and the read
   starts after its largest key, compared in Arrow's row format. A table missing at a
   version reads as empty.
2. Each chunk is compared with the current table at the start of the load: a row whose source
   columns equal the current row of its key is dropped; a deleted row whose key is not
   current is dropped too, since it changes nothing. What is left is appended and committed,
   then the reader is told. An empty chunk commits nothing.
3. The last commit reads the load's rows back from `__changes`, so the rows of a resumed load
   count. The current rows are those of the start without the load's keys, plus its rows that
   are not deletes. A due `compare` runs on them; its deletes, less those the load already
   holds, are appended with `_deleted = true`, and their keys leave the current table. The
   current table is replaced in the same commit.
4. `watermark_after` is `watermark_of` the load's rows, or the previous watermark when none
   has a value. The `ops.loads` row follows, through the ops adapter.
5. A failure before any commit of the load is recorded as `failed` with its code, `MG-0410`
   among them. After a commit, nothing is recorded: the load is unfinished and the next one
   resumes it (MG-REQ-0418.2).

## Retention

`expire_raw` calls the adapter's `expire` with the versions given as live and a rollback
window of `raw_retention`: the longest `retention` of the sources without `raw = false`, or
30 days. Retentions are read as `<n>m`, `<n>h` or `<n>d`, the form the source schema checks.

## Dependencies

- `arrow`: the record batches of raw, their keys and filters; the version of
  `mergus-adapter`.
- `mergus-adapter`: the `StorageAdapter` contract raw is written through.
- `mergus-adapter-ducklake`: the DuckLake adapter raw is stored in.
- `mergus-ports`: the `FileSystem`, `Database` and `Clock` ports, and `PortError`.
- `mergus-source`: `DeclaredTable` and the raw table names (MG-REQ-0403), and the
  incremental reads, deletes and very late rows of `load_table`.
- `mergus-project`: `Configuration`, and `key_line`, so that this crate parses no TOML
  (MG-REQ-0186).
- `serde_json`: the JSON values of `project.toml`.
- `thiserror`: derives `StorageProblem`.
- `mergus-sim`, as a dev-dependency only: the in-memory file system and the simulated
  `Database` of the tests. `mergus-ports` with `real`, as a dev-dependency only: the real
  DuckDB of the encryption test.
