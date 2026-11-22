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
| `Environment` | Command-line arguments, current directory, environment variables and the user configuration directory |
| `PortError` | `NotFound` or `Failed`, returned by every fallible call |
| `real::*` | `RealClock`, `RealEnvironment`, `RealFileSystem`, `RealProcess`, `RealRandom` (feature `real`) |

Every trait is `Send + Sync` and takes `&self`, so one value can be shared behind an `Arc`.

## Data flow

1. The wiring crate (`mergus-runtime`, `mergus-cli`, `mergus-py`) creates the real ports;
   tests and the simulator create the ports of `mergus-sim`.
2. It passes them, as constructor or context arguments, to the crates that need them.
3. Those crates call the traits only; they never name an implementation.

## Dependencies

- `thiserror`: derives `PortError`.
- The standard library, used directly only in `src/real/`, which allows Clippy's
  `disallowed_methods` and `disallowed_types` with a written reason (MG-REQ-0130).
- A dev-dependency on itself with the feature `real`, so `cargo test` runs the tests of the
  real implementations. Dev-dependencies are not linked into other crates.
