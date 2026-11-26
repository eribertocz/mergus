# mergus-source architecture

## Main types

| Item | Role |
|---|---|
| `check_sources` | Every problem of the sources of a `Configuration` |
| `SourceProblem` | One problem: code, severity, file, line, key and message |
| `Severity` | `Error`, which fails validation, or `Warning` |
| `SourceKind` | `database`, `files` or `rest` (MG-REQ-0401) |
| `Mode` | `copy`, `connect` or `live`, and the engines each allows for each kind (MG-REQ-0409) |
| `attach_duckdb_file`, `attach_postgres` | Attach a database source read-only and identify its engine (MG-REQ-0421, MG-REQ-0410) |
| `AttachedSource` | The connection, the alias and the `EngineIdentity` of an attached source |
| `DatabaseEngine` | `postgresql` or `duckdb`, the databases Mergus reads (MG-REQ-0409.4) |
| `ConnectError` | `EngineMismatch` (`MG-0404`) or a `PortError` |

## Data flow

1. The caller reads the project with `mergus_project::read_configuration`; only a valid
   configuration is checked, so every value has the type of its schema.
2. `check_sources` takes each source in the order of the configuration and reads its file
   again through the `FileSystem` port, only to find the line of each key with
   `mergus_project::key_line`.
3. For a source of a known kind it checks the `engine` against the `mode` (default `copy`),
   then each table that declares `incremental`: a missing `key` is an error, a missing
   `deletes` a warning. The line is that of `engine`, of `mode` when no engine is declared,
   or of `incremental`.
4. For a `copy` source of `kind = "database"` the engine is required and must be
   `postgresql` or `duckdb`; a missing one is located at `kind`.
5. The problems of a source are sorted by line, code and key, so their order does not
   depend on the order of the keys in memory (MG-REQ-0005).

## Attaching a database source

1. `attach_duckdb_file` and `attach_postgres` open a connection whose own database is in
   memory, so nothing is ever written to a DuckDB file of a source.
2. A DuckDB file is attached with the `Database` port's `attach_duckdb_file`; a refused
   attach is an engine that cannot be identified. Then `SELECT version()` gives the bundled
   DuckDB's version.
3. PostgreSQL is attached with the port's `attach_postgres`; a failure there is a
   `PortError`. Then `SELECT * FROM postgres_query(?, ?)` runs `SELECT version()` on the
   server; an answer that does not start with `PostgreSQL` is another engine, and a failed
   query an engine that cannot be identified. On a mismatch the source is detached.

## Dependencies

- `mergus-ports`: the `FileSystem` and `Database` ports and `PortError`.
- `arrow`: reads the text answer of a version query from its record batch.
- `mergus-project`: `Configuration` and `Source`, the validated source files, and
  `key_line`, the line of a key, so that this crate parses no TOML (MG-REQ-0186).
- `serde_json`: the JSON values of those files.
- `thiserror`: derives `SourceProblem`.
- `mergus-sim`, as a dev-dependency only: the in-memory file system of the tests.
