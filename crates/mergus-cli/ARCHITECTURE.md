# mergus-cli architecture

## Main types

| Item | Role |
|---|---|
| `run::Ports` | The ports one run uses: `Environment`, `FileSystem`, `Clock`, `Random`, `Keychain` |
| `run::run` | Parses the arguments, runs the command, reports, returns the exit code |
| `run::Answers` | Standard input when it is a terminal, which `init` asks its questions on |
| `run::SecretInput` | Standard input as `secret set` reads it: one hidden line on a terminal, all of it otherwise |
| `secret::secret_set` | `mergus secret set`: reads the value and stores it with `mergus-security` |
| `commands` | Every command and the arguments it accepts, defined once for the parser and `schema` |
| `args::parse` | The command line, read against `commands`; the closest valid word on error |
| `cli_schema::document` | The `mergus_cli` document of one command or of every command |
| `init::init` | The questions of `init` and the plan of the project it creates |
| `scaffold` | The plans of `new model` and `add source`, and the name checks |
| `files::Plan` | Files and folders to create: checked first, listed by `--dry-run`, then written |
| `mode` | The output mode (interactive, plain, events), colors and verbosity |
| `layout::render` | One error in the layout of MG-REQ-1322, with or without colors |
| `models::read` | The project for `ls`, `explain` and `plan`: options, identifier, code and inputs of each model |
| `selection::select` | The models a selection selects, each with its reasons (MG-REQ-1305) |
| `settings::max_parallel` | `max_parallel` from the flag, the variable, the user file or the default |
| `planning::plan` | `mergus plan`: the plan of `mergus-plan`, written under `.mergus/plans/` |
| `execution::run` | `mergus run` and `mergus backfill`: stages 1 to 5 through the run cycle of `mergus-runtime` |
| `work::work` | One `SqlWork` per `.sql` model the plan computes, its schema described by DuckDB |
| `arrow_types::schema` | The Arrow schema of the columns DuckDB describes |
| `idempotency` | The records of `--idempotency-key` under `.mergus/idempotency/`, for 24 hours |
| `doctor::doctor` | `mergus doctor`: the origin of each secret reference, from `mergus-security` |
| `explain::explain` | `mergus explain`: a code's catalog file, or a model and its options |
| `validate::validate` | Stage 1: layout, configuration and folder options of `mergus-project`, source checks of `mergus-source` |
| `project::find` | The nearest directory upwards that holds `project.toml` |
| `diagnostic::Diagnostic` | One error or warning: code, message, location, hint, next invocations, source line |
| `diagnostic::group` | Orders diagnostics by file, in order of first appearance |
| `error::CliError` | The errors of the binary itself, `MG-1301` to `MG-1308` |
| `catalog` | The files of `errors/`, compiled in: exit code, retryable flag, hint and cause of each code |
| `output::Output` | Events on standard output with `--json`, errors and stage lines on standard error otherwise |
| `exit` | The exit-code table and its retryable flags |
| `time::rfc3339` | Timestamps in UTC with milliseconds |
| `name::PRODUCT`, `name::env_prefix` | Names derived from `mergus_contracts::names::BINARY` |

## Data flow

1. `main` creates `RealEnvironment`, `RealFileSystem`, `RealClock`, `RealRandom` and
   `RealKeychain` and calls `run::run` with standard output and standard error. Tests call
   it with the ports of `mergus-sim` and buffers.
2. `run` reads the arguments from the `Environment` port and chooses the mode from them, from
   whether standard output is a terminal (`main` asks `std::io::IsTerminal`) and from `TERM`
   and `NO_COLOR`. `Output` draws the `run_id` from `Random` once.
3. An invalid command line gives one `MG-1301` diagnostic and `run_finished`, without
   `run_started`. Otherwise `run_started` comes first, then the command's diagnostics,
   grouped by file. `validate` attaches to each located diagnostic the text of its line.
4. Each diagnostic takes its exit code and, unless it has a more precise one, its hint from
   the catalog. The run ends with the highest exit code, 0 when there is no diagnostic. A
   warning, such as `MG-0403`, has exit code 0 and severity `warning`.
5. `schema` prints its document alone on standard output, without events, with or without
   `--json`. `init`, `new model` and `add source` build a `files::Plan`, refuse it when a
   name is invalid or taken or a file exists, then list it with `--dry-run` or write it.
   What they print for a person goes to standard output, or to standard error with `--json`.
6. `ls`, `explain <model>` and `plan` read the project with `models::read`; a project that
   `validate` would reject gives its diagnostics and nothing else. What `ls` and `explain`
   print, and the steps `plan` lists, go where `init` prints its next steps. `plan` writes
   only `.mergus/plans/<plan_id>.json` and emits `plan_ready` with `--json`.
7. `secret set` checks the name, reads the value from `run::SecretInput`, wraps it in a
   `SecretString` and calls `mergus_security::store` with the `Keychain` port. `main` gives
   `RealKeychain` and reads a terminal through `rpassword`, which turns echo off. A word
   other than the name gives an `MG-1301` that does not show the word, which may be the
   value.
8. `doctor` reads the configuration with `mergus_project::read_configuration`, collects the
   `secret:<name>` strings of `connection` and `replica` of every source into a sorted set,
   and asks `mergus_security::origin` for each. `origin` drops the value inside
   `mergus-security`; the binary only ever holds the `SecretOrigin`. A reference that does not
   resolve prints `missing` and becomes an `MG-1003` diagnostic with `secret set` as `next`.
9. `run` and `backfill` read the project as `plan` does, then through
   `mergus_runtime::read_project`, open the run cycle (`Run::begin`), plan with its complete
   candidates, check stage 4, map each computed `.sql` model onto a `SqlWork`, adopt the
   plan, emit `plan_ready`, execute, and always `finish`. Results stay as candidates in work.
   `main` sets the cancel token on Ctrl-C; a cancelled run ends with exit code 7.
10. A command given `--idempotency-key` runs into buffers, keeps its output and exit code in
   `.mergus/idempotency/<sha256(key)>.json` with the hash of its sorted arguments, then
   prints them. Within 24 hours, the same key and arguments print the record again and do
   nothing; other arguments give `MG-1314`.
11. Every event goes through `mergus_contracts::validate_event` before it is printed. An event
   that fails is an internal error, `MG-1304`, printed on standard error, exit code 10.

The `[[bin]] name` in `Cargo.toml` is the only other place that spells the product name:
Cargo cannot read a Rust constant. A unit test checks that it equals `name::PRODUCT`.

## Dependencies

- `mergus-contracts`: the product name and the `mergus_events` and `mergus_cli` contracts.
- `mergus-ports` with `real`: the ports and their real implementations.
- `mergus-project`: the checks of stage 1, the name rules, `closest_name`, the `$id` of
  the configuration schemas, the effective options, identifiers and references of models,
  and `toml_to_json`, which reads the user configuration file.
- `mergus-source`: the checks of the source files (MG-REQ-0409, MG-REQ-0414, MG-REQ-0416).
- `mergus-security`: `SecretRef` and `store`, which writes a value into the keychain.
- `rpassword`: reads the line typed on a terminal without echo, in `main` only.
- `mergus-plan`: the fingerprints and the `mergus_plan` document of `mergus plan`.
- `thiserror`: derives `CliError`.
- `mergus-runtime`: the run cycle, `SqlWork` and the cancel token of `run` and `backfill`.
- `arrow`: the declared schemas of the DuckDB segments.
- `ctrlc`: Ctrl-C sets the cancel token instead of ending the process (MG-REQ-0710.4).
- `serde_json`: the records of idempotency keys.
