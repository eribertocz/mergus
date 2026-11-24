# mergus-ports

I/O traits and their real implementations.

## What it does

- Defines one trait per port: `Clock`, `FileSystem`, `ObjectStore`, `Network`,
  `SourceConnector`, `Random`, `Process`, `Environment`, `SqlParser` and `Keychain`, and the
  `PortError` they return.
- Holds, under the non-default feature `real`, the real implementations that wrap the
  operating system: `RealClock`, `RealEnvironment`, `RealFileSystem`, `RealKeychain`, `RealProcess`
  and `RealRandom`, and `RealSqlParser`, DuckDB's parser in an in-memory DuckDB without external
  access that never executes the text (MG-REQ-0193).
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
- Its errors never carry a keychain value: `RealKeychain` keeps only the kind of a `keyring`
  error (MG-REQ-1002).
