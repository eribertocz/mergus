# mergus-engine-duckdb architecture

## Main types

| Item | Role |
|---|---|
| `Segment` | What a segment runs: SQL, parameters, macro texts, inputs and output |
| `Table` | An input the SQL reads by name, or the output, with its declared schema |
| `Boundary` | A boundary file and its `Transport` |
| `Transport`, `MEMORY_BUDGET` | Arrow in shared memory or Parquet in the work zone (MG-REQ-0633) |
| `run_segment` | Runs one segment in a new DuckDB session |
| `AS_OF` | The built-in table macro `as_of(table, ts)`, executed in every segment session (MG-REQ-0666) |
| `History`, `HistoryInput`, `ChangeDetection`, `Deletes` | One run of a history model: its table, input, key, `by`, `deletes` and detection time |
| `apply_history` | Applies a run to its history table in one transaction (MG-REQ-0662 to MG-REQ-0664, MG-REQ-0667) |
| `rebuild_history`, `RebuildHistory` | Drops a history table only for a confirmed rebuild (MG-REQ-0665) |
| `table_hash` | The hash of a table, independent of row order (MG-REQ-0638) |
| `EngineError` | `Unrepresentable` (`MG-0101`), `ZoneExtensionLoaded` (`MG-0102`), `HistoryRebuild` (`MG-0604`), `HistoryRepeatedKey` (`MG-0605`), `HistoryByType` (`MG-0606`), `HistoryNullKey` (`MG-0607`), `RolledBack`, `Boundary`, `Scratch`, `Hash` or a `PortError` |

## Data flow

```
Segment ─► check every declared schema                  (MG-REQ-0180)
        ─► Database::connect (in memory) ─► no `icu` loaded (duckdb_extensions())
        ─► execute AS_OF, then each macro text          (MG-REQ-0666, MG-REQ-0643)
        ─► no `icu` loaded
        ─► each input:
             SharedMemory: FileSystem::open ─► CREATE TABLE <name> ─► per Arrow IPC batch:
                           conform to schema (MG-REQ-0180) ─► append_arrow
             Parquet:      SET VARIABLE ─► DESCRIBE, check declared types (MG-REQ-0180)
                           ─► CREATE VIEW <name> AS SELECT <columns> FROM read_parquet(...)
        ─► DESCRIBE <SQL>, check declared or exactly widened types
        ─► no `icu` loaded ─► query_arrow_each(SQL, parameters), per batch:
             conform to the output schema, zones restored, then
             SharedMemory: write to the Arrow IPC file of FileSystem::create
                           (on failure: FileSystem::remove_file)
             Parquet:      append_arrow to mergus_output of a second session on
                           <output>.duckdb ─► COPY TO <path> ─► remove <output>.duckdb,
                           its .wal and its .tmp spill directory
```

```
History ─► BEGIN ─► DESCRIBE input (raw metadata columns left out)
        ─► SET VARIABLE detection time, sentinel (bound)
        ─► CREATE TABLE IF NOT EXISTS <history> (model columns + validity columns)
        ─► stage mergus_history_rows: changes view (ok loads past `after`, round = rank of
           the row in its key by _extracted_at, _load_id) or the whole input (round 1,
           keys unique or MG-0605); no NULL key (MG-0607)
        ─► per round: open versions (close the current one, insert the new one);
           changes view: deleted rows ─► close / tombstone
        ─► other input: missing keys ─► close / tombstone
        ─► COMMIT (ROLLBACK on error) ─► highest applied _load_id
```

`exact` maps each Arrow type to the type DuckDB describes for the column that holds it
exactly, converts a column only when every value survives the round trip unchanged, and puts
a declared time zone back on DuckDB's UTC instants. `transport` streams the Arrow IPC
files batch by batch; `output` streams the step's result to either transport. `hash` writes each row as canonical JSON with `mergus-contracts` and hashes
it with its SHA-256.

## Dependencies

- `mergus-ports`: the `Database` and `FileSystem` ports, with the streaming `open` and
  `create`.
- `mergus-contracts`: canonical JSON (RFC 8785) and SHA-256 (MG-REQ-0186).
- `arrow` with `ipc`: record batches, casts and the IPC file format.
- `serde_json`: the JSON value of a row; `thiserror`: the error type.
- Tests use `mergus-sim`, the simulated ports; `tests/real.rs` runs the same statements on
  the real `Database` and `FileSystem` of `mergus-ports`.
