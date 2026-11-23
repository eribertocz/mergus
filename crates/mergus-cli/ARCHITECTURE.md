# mergus-cli architecture

## Main types

| Item | Role |
|---|---|
| `run::Ports` | The ports one run uses: `Environment`, `FileSystem`, `Clock`, `Random` |
| `run::run` | Parses the arguments, runs the command, reports, returns the exit code |
| `args::parse` | The command line: `validate`, `--json`, `--quiet`, `--verbose`; the closest valid word on error |
| `mode` | The output mode (interactive, plain, events), colors and verbosity |
| `layout::render` | One error in the layout of MG-REQ-1322, with or without colors |
| `validate::validate` | Stage 1: layout, configuration and folder options of `mergus-project` |
| `project::find` | The nearest directory upwards that holds `project.toml` |
| `diagnostic::Diagnostic` | One error: code, message, location, hint, next invocations, source line |
| `diagnostic::group` | Orders diagnostics by file, in order of first appearance |
| `error::CliError` | The errors of the binary itself, `MG-1301` to `MG-1304` |
| `catalog` | The files of `errors/`, compiled in: exit code, retryable flag, hint and cause of each code |
| `output::Output` | Events on standard output with `--json`, errors and stage lines on standard error otherwise |
| `exit` | The exit-code table and its retryable flags |
| `time::rfc3339` | Timestamps in UTC with milliseconds |
| `name::PRODUCT`, `name::env_prefix` | Names derived from `mergus_contracts::names::BINARY` |

## Data flow

1. `main` creates `RealEnvironment`, `RealFileSystem`, `RealClock` and `RealRandom` and calls
   `run::run` with standard output and standard error. Tests call it with the ports of
   `mergus-sim` and buffers.
2. `run` reads the arguments from the `Environment` port and chooses the mode from them, from
   whether standard output is a terminal (`main` asks `std::io::IsTerminal`) and from `TERM`
   and `NO_COLOR`. `Output` draws the `run_id` from `Random` once.
3. An invalid command line gives one `MG-1301` diagnostic and `run_finished`, without
   `run_started`. Otherwise `run_started` comes first, then the command's diagnostics,
   grouped by file. `validate` attaches to each located diagnostic the text of its line.
4. Each diagnostic takes its exit code and, unless it has a more precise one, its hint from
   the catalog. The run ends with the highest exit code, 0 when there is no diagnostic.
5. Every event goes through `mergus_contracts::validate_event` before it is printed. An event
   that fails is an internal error, `MG-1304`, printed on standard error, exit code 10.

The `[[bin]] name` in `Cargo.toml` is the only other place that spells the product name:
Cargo cannot read a Rust constant. A unit test checks that it equals `name::PRODUCT`.

## Dependencies

- `mergus-contracts`: the product name and the `mergus_events` contract.
- `mergus-ports` with `real`: the ports and their real implementations.
- `mergus-project`: the checks of stage 1 and `closest_name`.
- `thiserror`: derives `CliError`.
