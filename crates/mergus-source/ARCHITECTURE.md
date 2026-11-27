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
| `SourceCare`, `Load`, `Budget` | The care options of a source with their defaults, and its budget (MG-REQ-0426, MG-REQ-0427) |
| `Brake`, `read_with_brake`, `SourceIncident` | The pace of the reads of a source, and the stop after three failures (`MG-0405`, MG-REQ-0428) |
| `Window`, `LoadKind`, `wait_for_window` | When a full load or a backfill may start (MG-REQ-0430) |
| `check_indexes`, `IndexWarning` | Keys and incremental columns without an index (`MG-0406`, MG-REQ-0431) |
| `InspectError`, `ImpactUnknown` | A source `doctor` cannot reach or read (`MG-0407`), and the warning of `plan` for one (`MG-0408`, MG-REQ-0432.2) |
| `source_impact`, `TableImpact` | What reading each table costs the source (MG-REQ-0432) |
| `PG_INDEXES`, `SCHEMA_INDEXES`, `PG_ESTIMATES`, `SCHEMA_ESTIMATES` | The fixed statements of the index check and the estimates, so that tests can record their answers |

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

## Taking care of the source

1. `SourceCare::of` reads `connection`, `replica`, `load`, `max_connections`, `window` and
   `query_timeout` of a validated source file, with their defaults for the environment.
2. `SourceCare::extraction_reference` is the secret reference the caller resolves: the
   replica's when there is one. `SourceCare::postgres_source` builds the `PostgresSource` of
   the `Database` port with the budget's connections and `query_timeout`, which the port
   sets as `pg_pool_max_connections` and `pg_statement_timeout_millis` (MG-REQ-0196.3).
3. `read_with_brake` reads the chunks of `read_table` with the `Brake`'s chunk rows, the
   budget's, sleeping on the `Clock` port for the
   `Brake`'s pause before each chunk but the first, and measuring each chunk on the
   monotonic clock. The running median is the lower middle of the sorted latencies of the
   chunks before; an extra pause below 1 ms is dropped so that halving ends. A failed chunk
   is sent again by `TableReader::retry`, from the same key.
4. `wait_for_window` reads the wall clock and sleeps until the window's start, in UTC, so
   that the result does not depend on the machine's time zone (MG-REQ-0005).
5. The index check and the estimates send a fixed PostgreSQL text through `postgres_query`
   once per source, and choose the schema with a bound parameter in the DuckDB query around
   it (MG-REQ-0007); the answer is grouped by table. A table is in the schema `public`, where
   DuckDB's `postgres` extension reads `<alias>.<table>`. An index covers columns when its
   leading columns are those columns, in their order (MG-REQ-0431.4).

## Dependencies

- `mergus-ports`: the `FileSystem`, `Database` and `Clock` ports and `PortError`.
- `arrow`: reads the text answer of a version query, and the key of the last row of a chunk.
- `mergus-project`: `Configuration` and `Source`, the validated source files, and
  `key_line`, the line of a key, so that this crate parses no TOML (MG-REQ-0186).
- `serde_json`: the JSON values of those files.
- `thiserror`: derives `SourceProblem`.
- `mergus-sim`, as a dev-dependency only: the in-memory file system of the tests.
