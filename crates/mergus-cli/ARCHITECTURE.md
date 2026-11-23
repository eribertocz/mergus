# mergus-cli architecture

## Main types

| Item | Role |
|---|---|
| `run::Ports` | The ports one run uses: `Environment`, `FileSystem`, `Clock`, `Random` |
| `run::run` | Parses the arguments, runs the command, reports, returns the exit code |
| `run::Answers` | Standard input when it is a terminal, which `init` asks its questions on |
| `commands` | Every command and the arguments it accepts, defined once for the parser and `schema` |
| `args::parse` | The command line, read against `commands`; the closest valid word on error |
| `cli_schema::document` | The `mergus_cli` document of one command or of every command |
| `init::init` | The questions of `init` and the plan of the project it creates |
| `scaffold` | The plans of `new model` and `add source`, and the name checks |
| `files::Plan` | Files and folders to create: checked first, listed by `--dry-run`, then written |
| `mode` | The output mode (interactive, plain, events), colors and verbosity |
| `layout::render` | One error in the layout of MG-REQ-1322, with or without colors |
| `validate::validate` | Stage 1: layout, configuration and folder options of `mergus-project` |
| `project::find` | The nearest directory upwards that holds `project.toml` |
| `diagnostic::Diagnostic` | One error: code, message, location, hint, next invocations, source line |
| `diagnostic::group` | Orders diagnostics by file, in order of first appearance |
| `error::CliError` | The errors of the binary itself, `MG-1301` to `MG-1308` |
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
5. `schema` prints its document alone on standard output, without events, with or without
   `--json`. `init`, `new model` and `add source` build a `files::Plan`, refuse it when a
   name is invalid or taken or a file exists, then list it with `--dry-run` or write it.
   What they print for a person goes to standard output, or to standard error with `--json`.
6. Every event goes through `mergus_contracts::validate_event` before it is printed. An event
   that fails is an internal error, `MG-1304`, printed on standard error, exit code 10.

The `[[bin]] name` in `Cargo.toml` is the only other place that spells the product name:
Cargo cannot read a Rust constant. A unit test checks that it equals `name::PRODUCT`.

## Dependencies

- `mergus-contracts`: the product name and the `mergus_events` and `mergus_cli` contracts.
- `mergus-ports` with `real`: the ports and their real implementations.
- `mergus-project`: the checks of stage 1, the name rules, `closest_name` and the `$id` of
  the configuration schemas.
- `thiserror`: derives `CliError`.
