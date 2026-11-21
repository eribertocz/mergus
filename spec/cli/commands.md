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

### MG-REQ-1313 · Idempotency keys

Every command that changes state MUST accept `--idempotency-key <key>`. A second call with the
same key and the same arguments MUST return the first call's result without new effects; the
same key with other arguments MUST fail with a typed error.

- Kind: behavior
- Verify: test `mg_req_1313_same_key_has_no_second_effect`
- Phase: 0.4
- Decision: 0010

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
