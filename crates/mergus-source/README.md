# mergus-source

Extract from databases (ADBC) and files.

## What it does

- Checks the source files `sources/*.toml` that `mergus-project` has read and validated
  against their schema (MG-REQ-0376), and returns every problem in one pass, each with its
  `MG-####` code, its file, its line and the key at fault:
  - `MG-0401`, an error: an incremental table declares no `key` (MG-REQ-0414);
  - `MG-0402`, an error: a `live` source whose `engine` is not `duckdb`, `ducklake`,
    `clickhouse`, `iceberg`, `delta` or `parquet`, a `connect` source whose `engine` is not
    `ducklake`, `iceberg` or `delta`, either one without `engine`, a `copy` source of
    `kind = "database"` whose `engine` is missing or not `postgresql` or `duckdb`, or a `copy`
    source of another kind with an `engine` (MG-REQ-0409);
  - `MG-0403`, a warning: an incremental table declares no `deletes`, so rows deleted in the
    source will not be seen (MG-REQ-0416).
- Names the kinds of a source (`SourceKind`, MG-REQ-0401) and its modes (`Mode`), with the
  engines each mode allows for each kind.
- Attaches a database source read-only to an in-memory DuckDB connection through the
  `Database` port: a DuckDB file with `ATTACH '<location>' (READ_ONLY)` (`attach_duckdb_file`,
  MG-REQ-0421), PostgreSQL with the `postgres` extension (`attach_postgres`). Before any data
  is read it identifies the engine that answers, by the attach of a DuckDB file or by
  `SELECT version()` through `postgres_query`, and returns the version to record; a mismatch,
  or an engine that cannot be identified, is `MG-0404` (MG-REQ-0410).

- Reads the tables a source declares under `[tables.<name>]`, and no other (MG-REQ-0402),
  from an attached source, and names the raw tables each lands in:
  `raw.<source>__<table>` and `raw.<source>__<table>__changes` (MG-REQ-0403). The select
  list is the caller's column list, with the key and incremental columns added; the
  incremental filter `"<incremental>" >= ?` is sent to the source with the caller's mark
  (MG-REQ-0404). A table without `incremental`, or read without a mark, is read completely
  (MG-REQ-0411). With a chunk size from the caller, a table with a `key` is read by keyset
  pagination, `WHERE "key" > ? ORDER BY "key" LIMIT ?`, one statement per chunk, never
  `OFFSET` (MG-REQ-0417). Every value is a bound parameter (MG-REQ-0007).
- Copies only `copy` sources: a `connect` or `live` source returns no table and reads
  nothing (MG-REQ-0405).
- Before reading a table, compares its columns (`DESCRIBE`) with those of its last load,
  given by the caller. A dropped or renamed column, or any type change in this phase, stops
  that table with `MG-0405`, naming each column, its old type and its new one; an added
  column does not (MG-REQ-0407).
- Records, from the `Clock` port, the start and the end in UTC of each table's read, and the
  spread of the source: the latest end minus the earliest start (MG-REQ-0419).

- Reads an incremental table chunk by chunk with `IncrementalReader`, whose position moves
  past a chunk only when the caller says it committed it (MG-REQ-0413, MG-REQ-0418). With
  the watermark of the last load, the read starts at `watermark − overlap` for a `DATE`,
  `TIMESTAMP` (read as UTC) or `TIMESTAMPTZ` column and at the watermark for any other
  (MG-REQ-0412.2); the overlap is the table's, else the source's, else `1h`. A resumed read
  starts after the key the caller gives (MG-REQ-0418.2). `watermark_of` gives the largest
  non-NULL incremental value of committed rows in the text DuckDB gives for
  `CAST(value AS VARCHAR)` (MG-REQ-0412.1, MG-REQ-0413).
- Detects deletes as declared (MG-REQ-0415): `column:<name>` splits each chunk into kept and
  deleted rows, `true` or any non-NULL value of a non-boolean column being a delete, and a
  source without the column is `MG-0406`, before any row is read; `compare` reads the key and
  incremental columns of the source, within `delete_window` back from the load's start, when
  `delete_check` is due, and returns the raw rows whose key is gone; `none`, or no `deletes`,
  detects nothing.
- Reports, from a `compare`, the source rows missing from raw whose incremental value is
  older than `watermark − overlap`, with the `MG-0407` warning and the suggested
  `mergus backfill` command (MG-REQ-0420). They are counted, not loaded.

## What it does not

- It does not check `kind` itself: the schema of `mergus-project` already rejects a missing
  or unknown kind with `MG-0305`.
- It does not write raw tables: `mergus-runtime` and the storage adapter do. It does not resolve the secret of a PostgreSQL source: the caller
  passes the resolved connection value. Remote DuckDB over Quack (`whoami()`) is phase 0.7.
- It does not store the columns of the last load, nor show the spread: the caller keeps the
  columns each read returns, and the run report shows the spread.
- It writes nothing and prints nothing: `mergus validate` reports its problems.
- It does not commit chunks, drop re-read rows that did not change, or store the watermark:
  `mergus-runtime` does, through the storage adapters.
- It runs no `mergus backfill`: the command comes with backfill of raw in a later phase.
