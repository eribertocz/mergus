---
id: cli-commands
title: Command map, first minute, selection, confirmation and configuration precedence
crate: mergus-cli
phase: "0.2"
status: draft
depends_on: [contracts-events]
---

# Commands

| Family | Commands | Defined in |
|---|---|---|
| Create | `init`, `new model`, `add source` | here, `extract/rest.md` |
| Build | `validate`, `plan`, `explain`, `preview`, `dev`, `run`, `backfill` | `run/`, here |
| Project | `config`, `suggest`, `mv`, `fmt`, `contract lock`, `ls` | `project/tools.md`, here |
| Environments | `diff`, `promote`, `rollback` | `environments/` |
| Operate | `status`, `why`, `lineage`, `audit`, `schedule`, `source test`, `source record` | `observability/`, `run/` |
| Recover | `backup`, `restore`, `restore-table`, `migrate`, `upgrade` | `ops/` |
| Security | `secret`, `doctor`, `audit verify`, `forget` | `security/` |
| Help | `explain <code>`, `schema`, `completions`, `docs`, `self update`, `bug-report` | here, `distribution/` |

### MG-REQ-1301 · init

`mergus init` MUST ask at most three questions (name, profile, gold location) with defaults,
create a valid project, and end by printing the next three suggested commands.

- Kind: behavior
- Verify: test `mg_req_1301_init_three_questions_three_next_steps`

### MG-REQ-1302 · init --demo

`mergus init --demo` MUST create a project with sample data that extracts, transforms,
publishes and validates its catalog against Menura in under 60 seconds on the reference
machine, without network access beyond Menura.

- Kind: behavior
- Verify: test `mg_req_1302_demo_runs_end_to_end`
- Phase: 0.5

### MG-REQ-1303 · init --from

`mergus init --from <connection>` MUST read the source schema and write the source file, one
staging model per table and PII suggestions for the user to confirm. PostgreSQL in the MVP.

- Kind: behavior
- Verify: test `mg_req_1303_init_from_postgres`
- Phase: 0.8

### MG-REQ-1304 · new model and add source

`mergus new model <name> [--sql|--py] [--gold <schema>]` MUST create the model file with a
minimal example in the right folder. `mergus add source <name>` MUST create
`sources/<name>.toml` with a secret reference.

- Kind: behavior
- Verify: test `mg_req_1304_scaffolding_commands`

### MG-REQ-1305 · Selection syntax

Selectors MUST support: `<model>`, `<model>+` (and downstream), `+<model>` (and upstream),
`+<model>+`, `tag:<t>`, `source:<s>`, `path:<folder>`, `changed` (fingerprint differs from
prod), union by space, intersection by comma, and `--exclude`. `mergus ls <selection>` prints
what is selected and why.

- Kind: behavior
- Verify: test `mg_req_1305_selectors`

### MG-REQ-1306 · No manual state

No command MUST require the user to pass a previous manifest or state file; comparison with
prod is automatic.

- Kind: behavior
- Verify: review `no-manual-state`

### MG-REQ-1307 · Irreversible actions confirm

`promote --to prod`, `rollback`, `forget`, `restore` and `secret rotate --pii` MUST show what
will change and ask the user to type the environment name. Without a terminal they MUST
require `--yes`, else exit 1 explaining how to continue. All of them MUST support
`--dry-run`.

- Kind: behavior
- Verify: test `mg_req_1307_irreversible_actions_confirm`
- Phase: 0.7

### MG-REQ-1308 · dev

`mergus dev` MUST watch the project, revalidate stages 1–4 within one second of a save, and
refresh the preview of a saved model on a sample in work. It MUST never publish.

- Kind: behavior
- Verify: test `mg_req_1308_dev_revalidates_and_never_publishes`
- Phase: 0.8

### MG-REQ-1309 · Configuration precedence

Settings MUST be resolved in this order: flags; `MERGUS_*` variables; the environment
section of `project.toml`; the rest of `project.toml`; `~/.config/mergus/config.toml`;
defaults. `mergus doctor` shows the origin of each value.

- Kind: behavior
- Verify: test `mg_req_1309_settings_precedence`

### MG-REQ-1310 · explain dispatches by argument

`mergus explain <argument>` MUST explain an error code when the argument matches
`MG-\d{4}`, and a model otherwise (`project/tools.md`).

- Kind: behavior
- Verify: test `mg_req_1310_explain_dispatch`

### MG-REQ-1311 · Completions

`mergus completions <bash|zsh|fish|powershell>` MUST print completions that include model,
environment, source and tag names dynamically.

- Kind: behavior
- Verify: test `mg_req_1311_dynamic_completions`
- Phase: 0.8

### MG-REQ-1312 · describe --json

`mergus describe --json` MUST print a versioned contract document with the models, tables,
columns (name, logical type, nullability) and actions the current identity is allowed to see or
run, and nothing else. An object the identity cannot see MUST be indistinguishable from one
that does not exist. The document MUST contain no data values and no secrets.

- Kind: behavior
- Verify: test `mg_req_1312_describe_shows_only_what_is_visible`
- Phase: 0.6
- Depends: MG-REQ-1404
- Decision: 0008

### MG-REQ-1314 · describe shows state

The document of MG-REQ-1312 MUST also give, for each visible model and table, the stable
identifiers of its upstream and downstream objects (MG-REQ-1122), its tests, its health with the
reason (MG-REQ-1121), the status and identifier of its last run, and the PII label of each column
(MG-REQ-1035), still with no data values.

- Kind: behavior
- Verify: test `mg_req_1314_describe_includes_state`
- Phase: 0.7
- Depends: MG-REQ-1312
- Decision: 0014

### MG-REQ-1315 · profile

`mergus profile <table> --json` MUST print, for a table the identity may read, its row count and
for each column the profile of MG-REQ-0818, plus the count of duplicate values of each declared
key. Minimum and maximum MUST be omitted for a column with a PII label, and columns the identity
cannot see MUST be absent.

- Kind: behavior
- Verify: test `mg_req_1315_profile_respects_pii_and_visibility`
- Phase: 0.7
- Depends: MG-REQ-0818, MG-REQ-1312
- Decision: 0014

### MG-REQ-1316 · run status

`mergus run status <run_id> --json` MUST print the run's status, its current step and its
`run_finished` event once it ended. With `--wait --timeout <duration>` it MUST return as soon as
the run ends, or at the timeout with the status at that moment, so an agent waits without
polling. The command exits 0 whenever it printed a status; the run's own exit code is in its
`run_finished` event.

- Kind: behavior
- Verify: test `mg_req_1316_run_status_waits_until_end_or_timeout`
- Phase: 0.7
- Decision: 0014

### MG-REQ-1317 · review

`mergus review <branch> --json` MUST print, without changing state, one versioned contract
document for a proposed change on `<branch>`: the project files changed against `main`, the
result of `validate`, the plan, the data diff of its branch environment against prod
(MG-REQ-1228) and the test results. It is what a host shows a person before approval
(MG-REQ-0191); approving, merging and reverting belong to the host.

- Kind: behavior
- Verify: test `mg_req_1317_review_reports_a_branch`
- Phase: 0.7
- Depends: MG-REQ-1208, MG-REQ-1228
- Decision: 0015

### MG-REQ-1344 · Finding the project

A command that reads a project MUST use the nearest directory, starting at the current
directory and walking up, that contains `project.toml`. When none exists, it MUST be an error
with exit code 1 whose hint names `mergus init`.

- Kind: behavior
- Verify: test `mg_req_1344_project_found_upwards`
- Phase: 0.2

### MG-REQ-1345 · validate

`mergus validate` MUST run stage 1 (`run/stages.md`), that is every check of `mergus-project`,
report every diagnostic found in one pass (MG-REQ-0702), touch no data and write no file. It
ends with exit code 0 when no diagnostic is an error, otherwise with the highest exit code
among its errors.

- Kind: behavior
- Verify: test `mg_req_1345_validate_reports_all_and_writes_nothing`
- Phase: 0.2

## Phase 0.2

### MG-REQ-1348 · The CLI schema document

The document of MG-REQ-0263 MUST be the contract `mergus_cli` in `schemas/mergus_cli/v1/`,
starting at `1.0.0`: `contract`, `contract_version`, `mergus_version` and `commands`, sorted by
`name` (`"new model"`). A command has `name`, `summary`, `arguments`, `exit_codes`, `json` and
`dry_run`; an argument has `name`, `kind` (`positional`, `option` or `flag`), `type`
(`string`, `integer`, `boolean`, `path` or `selection`), `required`, `repeatable` and,
when they exist, `default` (as a string) and `values`. `mergus schema` prints the document
alone on standard output, with or without `--json`.

- Kind: behavior
- Verify: test `mg_req_1348_cli_schema_document`
- Phase: 0.2
- Decision: 0027

### MG-REQ-1349 · init in phase 0.2

1. MG-REQ-1349.1: The questions and defaults are: name (the current folder's name, lowercased,
   with every character outside `[a-z0-9_]` turned into `_`), profile (only `local` in 0.2) and
   gold location (`gold`). `--name`, `--profile` and `--location` answer them; without a
   terminal, or with `--yes`, the defaults answer the rest.
2. MG-REQ-1349.2: It writes `project.toml` (`name` and `[gold]`), the folders `models/` and
   `sources/`, and a `.gitignore` holding `.mergus/`; it refuses, with exit code 1, a folder
   that already has `project.toml`.
3. MG-REQ-1349.3: It ends by printing `mergus new model first_model`, `mergus validate` and
   `mergus plan`, in that order.

- Kind: behavior
- Verify: test `mg_req_1349_init_defaults_files_and_next_steps`
- Phase: 0.2
- Decision: 0027

### MG-REQ-1350 · Scaffolding in phase 0.2

1. MG-REQ-1350.1: `new model` writes `models/<name>.sql`, or `models/gold/<schema>/<name>.sql`
   with `--gold`, containing `SELECT 1 AS id`; with `--py` the file is `.py` and holds one
   function named `<name>` returning a one-column Polars data frame. `--sql` is the default.
2. MG-REQ-1350.2: `add source` writes `sources/<name>.toml` with its `#:schema` line,
   `kind = "database"`, `engine = "postgresql"` (MG-REQ-0409.4) and
   `connection = "secret:<name>"`, in that order, so `mergus validate` accepts it.
3. MG-REQ-1350.3: An invalid (MG-REQ-0305) or taken (MG-REQ-0303) name, or an existing file,
   is an error with exit code 1. `init`, `new model` and `add source` accept `--dry-run`, which
   lists the files they would write and writes none.

- Kind: behavior
- Verify: test `mg_req_1350_scaffolding_files`
- Phase: 0.2
- Decision: 0027, 0040

### MG-REQ-1351 · Settings in phase 0.2

The only setting in 0.2 is `max_parallel` (integer from 1, default 4): flag `--max-parallel`,
variable `MERGUS_MAX_PARALLEL`, key `max_parallel` of the user configuration file. That file
is `config.toml` in `$XDG_CONFIG_HOME/mergus/`, or else `~/.config/mergus/` (Linux and macOS),
and in `%APPDATA%\mergus\` (Windows), read through the `Environment` port; a missing file is
empty, and an unknown key is an error with exit code 1. The `project.toml` levels of
MG-REQ-1309 apply when a setting has a key there.

- Kind: behavior
- Verify: test `mg_req_1351_settings_sources_and_file`
- Phase: 0.2
- Decision: 0027

### MG-REQ-1352 · changed and explain in phase 0.2

1. MG-REQ-1352.1: `changed` selects the models whose fingerprint is not an available result
   of `prod`; with no storage, every model.
2. MG-REQ-1352.2: `explain <model>` prints the model's name, identifier, file, inputs, the
   models that read it, and every effective option with value and origin (MG-REQ-0335);
   engines and published names arrive with their phases.
3. MG-REQ-1352.3: `explain <code>` prints the code's catalog file; an unknown code or model is
   an error naming the closest one (MG-REQ-0359).

- Kind: behavior
- Verify: test `mg_req_1352_changed_and_explain`
- Phase: 0.2
- Decision: 0027
