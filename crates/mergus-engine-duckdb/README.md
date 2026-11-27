# mergus-engine-duckdb

Execute DuckDB steps.

## What it does

- Runs one segment (`Segment`) in its own in-memory DuckDB session, reached only through the
  `Database` port (MG-REQ-0194): the step's SQL, its bound parameters (MG-REQ-0007), the
  project's macro texts, its input tables and its output table (`Table`), each with its
  declared schema.
- Executes every macro text in each session before anything else, so every session that runs
  a segment has the project's macros (MG-REQ-0643).
- Applies the transport the plan chose for each boundary (MG-REQ-0633, MG-REQ-0704):
  - `SharedMemory`: an Arrow IPC file in the run's work directory, written and read one
    record batch at a time through the streaming `FileSystem::create` and `FileSystem::open`
    of the port, each batch appended to DuckDB before the next is decoded;
  - `Parquet`: a Parquet file in the work zone, written by DuckDB's `COPY` and read lazily
    through a view over `read_parquet` of the declared columns.
  `Transport::for_size` gives the rule: shared memory up to `MEMORY_BUDGET`, 1 GiB of Arrow
  per boundary, and Parquet above it.
- Passes every table across a boundary in the types its schema declares, or fails the step
  with `MG-0101` (MG-REQ-0180): every declared schema is checked before the session opens
  (types DuckDB holds exactly, no column twice), a shared-memory input is conformed batch by
  batch, a Parquet input and the step's result are described and every declared column must
  have its declared type whatever the number of rows (a result column may also have a type
  the declared one widens exactly: a wider integer, `DOUBLE` for `FLOAT`, `DECIMAL(p',s)` for
  `DECIMAL(p,s)` with `p' >= p`), and the output is conformed before it is written. Decimals keep their precision, NULL stays NULL,
  and a value that would overflow or lose a digit is never rounded, truncated or cast.
- Fails with `MG-0102` when the session has the `icu` extension loaded, checked with
  `duckdb_extensions()` when the session opens, after the macros and before the result is
  read.
- Removes a shared-memory boundary file whose write failed, so that no reader finds it
  truncated.
- Keeps time zones: an input must carry its declared zone, and DuckDB's `TIMESTAMPTZ`, which
  keeps only the instant and gives it back in UTC (the port never loads `icu`), gets the
  declared zone back on the same instants, whatever the host's zone.
- Writes the declared columns even when the step returns no rows.
- Hashes a table independently of the order of its rows (`table_hash`): the SHA-256 of the
  sorted SHA-256 hashes of each row's canonical JSON, each value tagged with its Arrow type
  (MG-REQ-0638).

## What it does not

- It does not plan segments or choose a boundary's transport: `mergus-plan` does, and the
  plan records it.
- It does not wire a run: `mergus-runtime` maps the plan onto `Segment` and calls it.
- It does not read the `[resources]` section: the budget is fixed until phase 0.9.
- It does not memory-map Arrow IPC files. Reading streams them, so peak memory is one batch
  plus what DuckDB holds, with no `unsafe`.
- It does not stream the step's result: `query_arrow` returns every batch of it, for both
  transports. A shared-memory output stays within the 1 GiB budget because the plan sends
  larger boundaries to Parquet; a Parquet output is still gathered in memory once before
  `COPY` writes it (see the comment on #243).
- It prints nothing and keeps no state between segments.
