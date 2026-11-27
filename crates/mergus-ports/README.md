# mergus-ports

I/O traits and their real implementations.

## What it does

- Defines one trait per port: `Clock`, `FileSystem`, `ObjectStore`, `Network`,
  `SourceConnector`, `Random`, `Process`, `Environment`, `SqlParser`, `Python`, `Database` and
  `Keychain`, and the `PortError` they return.
- Defines `PythonTree`, the tree of a Python file the `Python` port returns: top-level
  functions with their parameters and returns, imports and calls, each with line and column
  (MG-REQ-0611).
- Holds, under the non-default feature `real`, the real implementations that wrap the
  operating system: `RealClock`, `RealEnvironment`, `RealFileSystem`, `RealKeychain`, `RealProcess`
  and `RealRandom`, and `RealSqlParser`, DuckDB's parser in an in-memory DuckDB without external
  access that never executes the text (MG-REQ-0193), and `RealPython`, which runs a Python
  interpreter as a child process with `-I -S` and exactly the environment its caller gives,
  and reads a file's tree with a bundled `ast` script, never importing it (MG-REQ-0611). Its
  `run` runs a Python program with arguments in a given working directory, with the same
  options and environment and no time limit, and returns its exit code and outputs
  (MG-REQ-0617.1).
- Holds `RealDatabase`, in-process DuckDB that runs SQL with bound parameters, reads and
  writes Arrow, attaches DuckLake catalogs, and attaches database sources read-only: DuckDB
  files with no extension, and PostgreSQL with the `postgres` extension, its pool and
  statement timeout set from the source and its connection value in a temporary secret
  dropped on detach (MG-REQ-0196). It never installs or loads an extension on its own and
  refuses unsigned ones; it installs only `ducklake` and `postgres`, from
  `extensions.duckdb.org`, into `mergus/duckdb/extensions/` under the user cache directory
  the first time, loads them from there afterwards, and records the version DuckDB reports
  for each loaded extension (MG-REQ-0194).
- Is the only crate allowed to depend on `duckdb` (MG-REQ-0194).
- Is the only crate allowed to call the standard library's clock, file, environment, network
  and process functions (MG-REQ-0130); Clippy forbids them everywhere else.

## What it does not

- It holds no simulated implementation: those live in `mergus-sim` (MG-REQ-0131).
- It keeps no global or thread-local port: callers create ports and pass them explicitly
  (MG-REQ-0132).
- It does not enable `real` by default; only `mergus-runtime`, `mergus-cli`, `mergus-py` and
  `mergus-testkit` may enable it (MG-REQ-0133, MG-REQ-0126.5).
- It has no real `ObjectStore`, `Network` or `SourceConnector` yet; they come with the crates
  that need them.
- `RealRandom` is fit for identifiers, not for secrets.
- It never embeds Python, and does not build the Python environment: the caller builds it
  with `mergus_security::python_command`. Its port errors carry no `MG-####` code; the
  caller maps them.
- Its errors never carry a keychain value: `RealKeychain` keeps only the kind of a `keyring`
  error (MG-REQ-1002).
