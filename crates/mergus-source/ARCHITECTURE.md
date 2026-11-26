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
| `declared_tables`, `DeclaredTable` | The tables a source declares, with their key and incremental column (MG-REQ-0402) |
| `RawTables` | The raw tables a declared table lands in (MG-REQ-0403) |
| `read_table`, `TableReader` | The chunks of one table, read on demand (MG-REQ-0404, MG-REQ-0411, MG-REQ-0417) |
| `ReadOptions`, `Projection` | The caller's columns, incremental mark and chunk size |
| `extract_source`, `TableExtraction` | Every declared table of a source, with its raw tables |
| `ConnectError` | `EngineMismatch` (`MG-0404`) or a `PortError` |
| `copied_tables` | The tables copied into raw: none of a `connect` or `live` source (MG-REQ-0405) |
| `source_columns`, `SourceColumn` | The columns of a source table and their DuckDB types, from `DESCRIBE` |
| `breaking_changes`, `ColumnChange` | The dropped, renamed or retyped columns since the last load (MG-REQ-0407) |
| `SchemaChange` | A table stopped by breaking changes (`MG-0405`) |
| `extract_source_timed`, `SourceExtraction`, `TableRead` | A source read with its changes checked and each table's read times and spread (MG-REQ-0419) |

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

## Reading a table

1. `declared_tables` reads the `tables` object of the validated source file, sorted by
   name; a `key` is one column or a list.
2. `read_table` builds `SELECT <columns> FROM "<alias>"."<table>"`. The columns are `*`, or
   the caller's list followed by the key and incremental columns it lacks, each quoted.
3. When the table declares `incremental` and the caller gives a mark, `"<incremental>" >= ?`
   is added; the mark is inclusive so that a row at the mark is never missed.
4. When the caller gives a chunk size and the table has a key, each chunk adds
   `ORDER BY <key> LIMIT ?` and, after the first, `<key> > ?` (a row comparison for a
   composite key) bound to the key of the last row read. A chunk shorter than the size is
   the last. Each chunk is one statement, so no transaction outlives it.
5. A key value is bound with its Arrow type when `SqlValue` has one, and as text otherwise.

## Source changes and read times

1. `copied_tables` is `declared_tables` for a `copy` source, the default, and empty for a
   `connect` or `live` source; `extract_source` and `extract_source_timed` read only these,
   so such a source returns no table and sends no statement (MG-REQ-0405).
2. For each copied table, `extract_source_timed` first runs
   `SELECT column_name, column_type FROM (DESCRIBE "<alias>"."<table>")`, which resolves the
   table as the read does, and compares the columns with those of the last load the caller
   gives. Each column of the last load that is missing, or whose type text differs, is a
   `ColumnChange`; an added column is not one, nor is anything at a first load (empty last
   columns). A table with changes becomes a `SchemaChange` and is not read; the other
   tables go on (MG-REQ-0407).
3. The wall clock of the `Clock` port is read just before the first data statement of a
   table and just after its last one (MG-REQ-0419.1). `spread` is the latest end minus the
   earliest start, zero if the wall clock went backwards (MG-REQ-0419.2). No official DuckDB
   page documents a snapshot shared by several attached PostgreSQL tables, so every source
   records times (MG-REQ-0419.3).

## Dependencies

- `mergus-ports`: the `FileSystem`, `Database` and `Clock` ports and `PortError`.
- `arrow`: reads the text answer of a version query, and the key of the last row of a chunk.
- `mergus-project`: `Configuration` and `Source`, the validated source files, and
  `key_line`, the line of a key, so that this crate parses no TOML (MG-REQ-0186).
- `serde_json`: the JSON values of those files.
- `thiserror`: derives `SourceProblem`.
- `mergus-sim`, as a dev-dependency only: the in-memory file system of the tests.
