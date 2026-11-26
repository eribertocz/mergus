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

## What it does not

- It does not check `kind` itself: the schema of `mergus-project` already rejects a missing
  or unknown kind with `MG-0305`.
- It does not extract anything yet, nor resolve the secret of a PostgreSQL source: the caller
  passes the resolved connection value. Remote DuckDB over Quack (`whoami()`) is phase 0.7.
- It writes nothing and prints nothing: `mergus validate` reports its problems.
