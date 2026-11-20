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
