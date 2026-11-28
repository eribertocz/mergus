# mergus-ports architecture

## Main types

| Type | Role |
|---|---|
| `Clock` | Wall-clock `Timestamp`, monotonic `MonotonicTime`, sleeping |
| `FileSystem` | Read, write, remove, create and list local files and directories, remove a directory tree; stream a file with `open` (a `SeekRead`) and `create` |
| `ObjectStore` | Get, put, delete and list objects by key |
| `Network` | HTTP requests (`HttpRequest`, `HttpResponse`) and TCP `Connection`s |
| `SourceConnector` | Opens a source as a `SourceReader` that yields batches of bytes |
| `Random` | Random numbers and bytes |
| `Process` | Runs a `ProcessCommand` and returns its `ProcessOutput` |
| `Environment` | Command-line arguments, current directory, environment variables and the user configuration and cache directories |
| `PortError` | `NotFound`, `Failed`, `ExtensionUnavailable` (extension, host, cache directory), `PythonNotFound` (what was tried), `PythonSyntax` (message, line, column) or `TimedOut` (program, limit), among others, returned by every fallible call |
| `SqlParser` | Returns the tree DuckDB's `json_serialize_sql` gives for a SQL text, without executing it |
| `Python`, `PythonTree` | Returns the tree of a Python file without importing it: `PythonFunction`s with `PythonParameter`s and `PythonReturn`s, `PythonImport`s and `PythonCall`s with their receiver `PythonSegment`s; `PythonTree::from_json` reads the document the interpreter prints. `run` runs a Python program with arguments in a working directory and returns a `ProcessOutput` |
| `Database`, `DatabaseConnection` | Connects to a `DatabaseLocation`; runs SQL with bound `SqlValue`s, returns, streams (`query_arrow_each`, DuckDB's streaming execution in the real port) and appends Arrow `RecordBatch`es, attaches a `DuckLakeCatalog`, a DuckDB file or a `PostgresSource` read-only, detaches them, loads an `Extension` and lists each `LoadedExtension` with its version |
| `Keychain` | Get, set and delete operating system keychain entries by service and account |
| `real::*` | `RealClock`, `RealDatabase`, `RealEnvironment`, `RealFileSystem`, `RealKeychain`, `RealProcess`, `RealPython`, `RealRandom`, `RealSqlParser` (feature `real`) |

Every trait is `Send + Sync` and takes `&self`, so one value can be shared behind an `Arc`.

## Data flow

1. The wiring crate (`mergus-runtime`, `mergus-cli`, `mergus-py`) creates the real ports;
   tests and the simulator create the ports of `mergus-sim`.
2. It passes them, as constructor or context arguments, to the crates that need them.
3. Those crates call the traits only; they never name an implementation.
4. `RealPython::locate` takes the `[python] interpreter` setting, else `<root>/.venv`, else
   `python3` on the given search path (`python` too on Windows), skipping empty files such as
   the Store's alias. `parse` runs it with
   `-I -S -c` and the script `src/real/python_tree.py` (`include_str!`), writes the source to
   its standard input, clears the inherited environment and sets only the one given. A
   call that runs longer than `PYTHON_TIMEOUT` (60 s) kills the child and fails with
   `TimedOut`. The script walks the tree without recursion, parses with `ast` and prints the
   JSON tree, which `PythonTree::from_json` reads. `run` starts the same interpreter with
   `-I -S -c <script> <args>`, the given working directory, the same environment and an
   empty standard input, reads both outputs on their own threads and waits for the child
   with no time limit.

## Dependencies

- `thiserror`: derives `PortError`.
- `serde` (with `derive`) and `serde_json`: read and write the JSON tree of the `Python`
  port, and compare trees as JSON values in tests.
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
- A dev-dependency on itself with the feature `real`, so `cargo test` runs the tests of the
  real implementations. Dev-dependencies are not linked into other crates.
