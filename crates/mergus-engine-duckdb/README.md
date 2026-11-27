# mergus-engine-duckdb

Execute DuckDB steps.

## What it does

- Runs one segment (`Segment`) in its own in-memory DuckDB session, reached only through the
  `Database` port (MG-REQ-0194): the step's SQL, its bound parameters (MG-REQ-0007), the
  project's macro texts and its inputs with their schemas.
- Executes every macro text in each session before anything else, so every session that runs
  a segment has the project's macros (MG-REQ-0643).
- Applies the transport the plan chose for each boundary (MG-REQ-0633, MG-REQ-0704):
  - `SharedMemory`: an Arrow IPC file in the run's work directory, written and read through
    the `FileSystem` port;
  - `Parquet`: a Parquet file in the work zone, written by DuckDB's `COPY` and read lazily
    through a view over `read_parquet`.
  `Transport::for_size` gives the rule: shared memory up to `MEMORY_BUDGET`, 1 GiB of Arrow
  per boundary, and Parquet above it.
- Loads a shared-memory input in the types its schema declares, or fails the step with
  `MG-0101` before its SQL runs: decimals keep their precision, NULL stays NULL, timestamps
  keep their time zone, and a value that would overflow, lose a digit or change its zone is
  never rounded, truncated or cast (MG-REQ-0180).
- Hashes a table independently of the order of its rows (`table_hash`): the SHA-256 of the
  sorted SHA-256 hashes of each row's canonical JSON (MG-REQ-0638).

## What it does not

- It does not plan segments or choose a boundary's transport: `mergus-plan` does, and the
  plan records it.
- It does not wire a run: `mergus-runtime` maps the plan onto `Segment` and calls it.
- It does not read the `[resources]` section: the budget is fixed until phase 0.9.
- It does not check a Parquet input against its schema: DuckDB wrote it with its own types.
- It prints nothing and keeps no state between segments.
