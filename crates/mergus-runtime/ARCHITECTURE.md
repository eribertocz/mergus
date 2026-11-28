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
| `read_project`, `ReadPorts`, `ReadProject`, `ReadModel` | Stage 1: the read project |
| `ReadError`, `FileErrors` | Every error of stage 1, grouped by file |
| `plan_project`, `plan_models` | Stages 2 and 3, with the available candidates |
| `Run` | One run: its scratch directory, candidates and stage 5 |
| `Work`, `ModelWork`, `StepContext`, `Extraction` | What stage 5 runs beyond the plan |
| `SqlWork`, `SqlInput` | The work of one DuckDB segment, by `mergus-engine-duckdb` |
| `Executed` | How stage 5 ended, and its exit code |
| `CancelToken`, `CANCELLED_EXIT_CODE` | The cancellation of a run, shared by every clone |
| `RunError`, `StepFailure` | Why a run failed, with its code |

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
5. A failure before any commit of the load is recorded as `failed` with its code, `MG-0411`
   among them. After a commit, nothing is recorded: the load is unfinished and the next one
   resumes it (MG-REQ-0418.2). A key read twice in the load, across its chunks and those of
   before a resume, is `MG-0504`, recorded `failed` even after commits; readers see only
   loads recorded `ok` (MG-REQ-0518.1, MG-REQ-0518.3).
6. A resumed load whose last commit was made, its rows current with its load id and its
   deleted keys gone, only gets its `ops.loads` row (MG-REQ-0418.2).
7. `rows` is the rows read; `deletes_checked_at` is the load's start when it compared, else
   NULL; the last compare is the latest non-NULL value (MG-REQ-0415.3).

## Retention

`expire_raw` calls the adapter's `expire` with the versions given as live and a rollback
window of `raw_retention`: the longest `retention` of the sources without `raw = false`, or
30 days. Retentions are read as `<n>m`, `<n>h` or `<n>d`, the form the source schema checks.

## The run cycle

1. `read_project` runs every pass of `mergus-project` whose inputs are valid: layout,
   configuration, `ids.toml`, folder options, `.sql` references and cycles, seeds, `lib/`
   and `.py` models when the `Python` port is given, macros when a `Database` is given. The
   errors are grouped by their path; groups are sorted by top folder (`models/`, `sources/`,
   `seeds/`, then the rest) and then entry by entry, and each group by line and column, with
   the errors of the whole file first, keeping the pass order between equals.
2. `Run::begin` lists `.mergus/work/runs/` and deletes every entry. An entry is a directory
   when the port can list it. It then deletes each candidate without `complete.json`.
3. Stage 5 runs on scoped threads. The main thread starts the first pending step, in plan
   order, whose inputs that are steps of the plan have finished, while fewer than
   `max_parallel` run and no step has failed; it checks the token before each. A step
   creates `steps/<model>/` in the scratch directory, runs its work, which leaves an Arrow
   IPC file there, starts its candidate afresh, copies the file batch by batch and completes
   the candidate. After the first failure no step starts; those running finish, and the
   first failure is returned.
4. `Run::finish` deletes the incomplete candidates, then the scratch directory.

## Dependencies

- `arrow`: the record batches of raw, their keys and filters, and the Arrow IPC files of the
  candidates; the version of `mergus-adapter`.
- `mergus-adapter`: the `StorageAdapter` contract raw is written through.
- `mergus-adapter-ducklake`: the DuckLake adapter raw is stored in.
- `mergus-ports`: the `FileSystem`, `Database` and `Clock` ports, and `PortError`.
- `mergus-source`: `DeclaredTable` and the raw table names (MG-REQ-0403), and the
  incremental reads, deletes and very late rows of `load_table`.
- `mergus-project`: `Configuration`, and `key_line`, so that this crate parses no TOML
  (MG-REQ-0186).
- `serde_json`: the JSON values of `project.toml`, and `complete.json`.
- `mergus-plan`: the plan, `PlanModel` and the `Cleared` stage 5 needs.
- `mergus-contracts`: the steps of the plan document.
- `mergus-engine-duckdb`: `run_segment`, which `SqlWork` calls.
- `thiserror`: derives `StorageProblem`.
- `mergus-sim`, as a dev-dependency only: the in-memory file system and the simulated
  `Database` of the tests. `mergus-ports` with `real`, as a dev-dependency only: the real
  DuckDB of the encryption test.
