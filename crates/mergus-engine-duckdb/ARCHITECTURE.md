# mergus-engine-duckdb architecture

## Main types

| Item | Role |
|---|---|
| `Segment` | What a segment runs: SQL, parameters, macro texts, inputs and output |
| `Input` | A boundary the SQL reads by name, with its declared schema |
| `Boundary` | A boundary file and its `Transport` |
| `Transport`, `MEMORY_BUDGET` | Arrow in shared memory or Parquet in the work zone (MG-REQ-0633) |
| `run_segment` | Runs one segment in a new DuckDB session |
| `table_hash` | The hash of a table, independent of row order (MG-REQ-0638) |
| `EngineError` | `Unrepresentable` (`MG-0101`), `Boundary`, `Hash` or a `PortError` |

## Data flow

```
Segment ─► Database::connect (in memory)
        ─► execute each macro text                      (MG-REQ-0643)
        ─► each input:
             SharedMemory: FileSystem::read ─► Arrow IPC ─► conform to schema (MG-REQ-0180)
                           ─► CREATE TABLE <name> ─► append_arrow
             Parquet:      SET VARIABLE ─► CREATE VIEW <name> AS read_parquet(...)
        ─► query_arrow(SQL, parameters)
        ─► output:
             SharedMemory: Arrow IPC ─► FileSystem::write
             Parquet:      CREATE TABLE mergus_output ─► append_arrow ─► COPY TO <path>
```

`exact` maps each Arrow type to the DuckDB type that holds it exactly and converts a column
only when every value survives the round trip unchanged. `transport` encodes and decodes the
Arrow IPC files. `hash` writes each row as canonical JSON with `mergus-contracts` and hashes
it with its SHA-256.

## Dependencies

- `mergus-ports`: the `Database` and `FileSystem` ports.
- `mergus-contracts`: canonical JSON (RFC 8785) and SHA-256 (MG-REQ-0186).
- `arrow` with `ipc`: record batches, casts and the IPC file format.
- `serde_json`: the JSON value of a row; `thiserror`: the error type.
- Tests use `mergus-sim`, the simulated ports.
