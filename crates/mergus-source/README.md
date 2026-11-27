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

- Takes care of the source (`SourceCare`), so that Mergus is never the reason it becomes
  slow:
  - every extraction query goes through the `replica` secret reference when it is set, and
    through `connection` otherwise (MG-REQ-0426);
  - `load` sets the budget of an extraction from this table, and the connections used are
    never more than `max_connections` (MG-REQ-0427). `load` defaults to `low` in `prod` and
    to `normal` elsewhere, `max_connections` to 2, `query_timeout` to `10m`. A PostgreSQL
    source is attached with a pool of the budget's connections and the statement timeout of
    `query_timeout`.

    | `load` | Connections | Chunk rows | Pause between chunks |
    |---|---|---|---|
    | `low` | 1 | 10 000 | 1 s |
    | `normal` | 2 | 50 000 | 200 ms |
    | `high` | 4 | 100 000 | none |

  - `read_with_brake` reads in chunks of the budget's rows, never a size the caller gives,
    and measures the latency of each chunk. Above twice the running median of
    the chunks before it, the pause before the next chunk doubles, or becomes the chunk's
    latency when it is zero; each chunk at most the median halves the pause above the
    budget's, until it is back. A chunk statement that fails or times out is sent again;
    after three consecutive failures the extraction of the source stops with `MG-0406`, an
    error, and is not retried until the next trigger (MG-REQ-0428);
  - `wait_for_window` holds a full load or a backfill until the source's `window` opens;
    the window is in UTC, spans midnight when its end is before its start, and is open all
    day when its start is its end (MG-REQ-0430.1). An incremental load is not held
    (MG-REQ-0430);
  - `check_indexes` gives `MG-0407`, a warning, for each key or incremental column of a
    PostgreSQL source without an index led by it: by the key columns in key order, or by the
    incremental column (MG-REQ-0431.4). The indexes are read from `pg_index` through
    `postgres_query`, once per source, and the warning gives the index to create:
    `CREATE INDEX ON "public"."<table>" ("<column>", …);` (MG-REQ-0431). A DuckDB file is
    not checked;
  - `source_impact` gives, per declared table, the rows the source estimates it holds,
    shown as "up to N" (`reltuples` for PostgreSQL, `estimated_size` for a DuckDB file,
    MG-REQ-0432.1), the index a read uses (one led by the incremental column, or by the key),
    the window and the budget, for `mergus plan` (MG-REQ-0432). The estimates and indexes are
    read once per source;
  - a source that `doctor --source` cannot reach, or whose catalog it cannot read, is
    `MG-0408`, an error (`InspectError`); one that `plan` cannot reach or read is `MG-0409`,
    a warning (`ImpactUnknown`) with the reason, and its tables show `unknown`
    (MG-REQ-0432.2).
- Reads an incremental table chunk by chunk with `IncrementalReader`, whose position moves
  past a chunk only when the caller says it committed it (MG-REQ-0413, MG-REQ-0418). With
  the watermark of the last load, the read starts at `watermark − overlap` for a `DATE`,
  `TIMESTAMP` (read as UTC) or `TIMESTAMPTZ` column, the overlap counted in whole days rounded
  up for a `DATE`, and at the watermark for any other (MG-REQ-0412.2); the overlap is the
  table's, else the source's, else `1h`. A resumed read
  starts after the key the caller gives (MG-REQ-0418.2). `IncrementalReader::watermark`
  gives the largest non-NULL incremental value of committed rows in the text DuckDB itself
  gives for `CAST(value AS VARCHAR)`, asked through the port (MG-REQ-0412.1, MG-REQ-0413).
- Detects deletes as declared (MG-REQ-0415): `column:<name>` splits each chunk into kept and
  deleted rows, `true` or any non-NULL value of a non-boolean column being a delete, and a
  source without the column is `MG-0411`, before any row is read; `compare` reads the key and
  incremental columns of the source, within `delete_window` back from the load's start, when
  `delete_check` is due, and returns the raw rows whose key is gone; `none`, or no `deletes`,
  detects nothing. `compare` without `key`, and `delete_window` without `incremental`, are
  `MG-0413` errors of `mergus validate`; a `delete_window` whose incremental column is not a
  date or time is `MG-0413` when the table is read (MG-REQ-0415.4).
- Reports, from a `compare`, the source rows missing from raw whose incremental value is
  older than `watermark − overlap`, with the `MG-0412` warning and the suggested
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
