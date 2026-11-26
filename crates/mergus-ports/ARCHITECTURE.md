# mergus-ports architecture

## Main types

| Type | Role |
|---|---|
| `Clock` | Wall-clock `Timestamp`, monotonic `MonotonicTime`, sleeping |
| `FileSystem` | Read, write, remove, create and list local files and directories |
| `ObjectStore` | Get, put, delete and list objects by key |
| `Network` | HTTP requests (`HttpRequest`, `HttpResponse`) and TCP `Connection`s |
| `SourceConnector` | Opens a source as a `SourceReader` that yields batches of bytes |
| `Random` | Random numbers and bytes |
| `Process` | Runs a `ProcessCommand` and returns its `ProcessOutput` |
| `Environment` | Command-line arguments, current directory, environment variables and the user configuration and cache directories |
| `PortError` | `NotFound`, `Failed` or `ExtensionUnavailable` (extension, host, cache directory), returned by every fallible call |
| `SqlParser` | Returns the tree DuckDB's `json_serialize_sql` gives for a SQL text, without executing it |
| `Database`, `DatabaseConnection` | Connects to a `DatabaseLocation`; runs SQL with bound `SqlValue`s, returns and appends Arrow `RecordBatch`es, attaches a `DuckLakeCatalog`, a DuckDB file or a `PostgresSource` read-only, detaches them, loads an `Extension` and lists each `LoadedExtension` with its version |
| `Keychain` | Get, set and delete operating system keychain entries by service and account |
| `real::*` | `RealClock`, `RealDatabase`, `RealEnvironment`, `RealFileSystem`, `RealKeychain`, `RealProcess`, `RealRandom`, `RealSqlParser` (feature `real`) |

Every trait is `Send + Sync` and takes `&self`, so one value can be shared behind an `Arc`.

## Data flow

1. The wiring crate (`mergus-runtime`, `mergus-cli`, `mergus-py`) creates the real ports;
   tests and the simulator create the ports of `mergus-sim`.
2. It passes them, as constructor or context arguments, to the crates that need them.
3. Those crates call the traits only; they never name an implementation.

## Dependencies

- `thiserror`: derives `PortError`.
- `arrow` (default features off): the record batches of the `Database` port, at the version
  `duckdb` uses.
- `duckdb` (feature `real` only; features `bundled`, `json`, `appender-arrow` and
  `parquet`, default features off): the in-memory DuckDB of `RealSqlParser` (MG-REQ-0193)
  and the DuckDB of `RealDatabase` (MG-REQ-0194). Bundles DuckDB v2.0.1, which supports
  DuckLake, with Parquet linked in so DuckLake can write its data files.
- `keyring` 3 (feature `real` only; default features off): `RealKeychain` on Keychain
  Services, Credential Manager and the Secret Service through `dbus`, with libdbus built from
  source (`vendored`) and pure-Rust cryptography, so no system library is needed to build.
  It is the only crate allowed to depend on `keyring` (MG-REQ-0126.7).
- The standard library, used directly only in `src/real/`, which allows Clippy's
  `disallowed_methods` and `disallowed_types` with a written reason (MG-REQ-0130).
- A dev-dependency on `serde_json`, to compare trees as JSON values in tests.
- A dev-dependency on itself with the feature `real`, so `cargo test` runs the tests of the
  real implementations. Dev-dependencies are not linked into other crates.
