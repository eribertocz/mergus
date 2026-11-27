# mergus-engine-duckdb architecture

## Main types

| Item | Role |
|---|---|
| `Segment` | What a segment runs: SQL, parameters, macro texts, inputs and output |
| `Table` | An input the SQL reads by name, or the output, with its declared schema |
| `Boundary` | A boundary file and its `Transport` |
| `Transport`, `MEMORY_BUDGET` | Arrow in shared memory or Parquet in the work zone (MG-REQ-0633) |
| `run_segment` | Runs one segment in a new DuckDB session |
| `table_hash` | The hash of a table, independent of row order (MG-REQ-0638) |
| `EngineError` | `Unrepresentable` (`MG-0101`), `ZoneExtensionLoaded` (`MG-0102`), `Boundary`, `Scratch`, `Hash` or a `PortError` |

## Data flow

```
Segment ─► check every declared schema                  (MG-REQ-0180)
        ─► Database::connect (in memory) ─► no `icu` loaded (duckdb_extensions())
        ─► execute each macro text                      (MG-REQ-0643)
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
