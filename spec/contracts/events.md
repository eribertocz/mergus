---
id: contracts-events
title: The --json event stream and exit codes
crate: mergus-cli
phase: "0.2"
status: draft
depends_on: [contracts-overview]
---

# Events and exit codes

## 1. Event stream

With `--json`, every command prints NDJSON events to standard output, one JSON object per
line, valid against `schemas/mergus-events.schema.json`. Human diagnostics go to standard
error.

| Event `type` | Emitted when | Required fields besides the common ones |
|---|---|---|
| `run_started` | A command starts | `command`, `arguments`, `environment` |
| `plan_ready` | The plan is computed | `steps[]` with model, action, reason |
| `step_started` | A step starts | `step_id`, `model`, `engine` |
| `step_finished` | A step ends | `step_id`, `status`, `rows`, `bytes`, `duration_ms` |
| `diagnostic` | An error or warning is found | `code`, `severity`, `message`, `location`, `hint`; optional `next` (MG-REQ-1334) |
| `quality_result` | A test or check ends | `test`, `model`, `status`, `failing_rows` |
| `published` | The pointer moved | `environment`, `manifest_id` |
| `run_finished` | The command ends | `status`, `exit_code`, `duration_ms` |

Common fields of every event: `contract`, `contract_version`, `type`, `run_id`, `sequence`,
`timestamp`.

### MG-REQ-0260 · Events are a public contract

`--json` output MUST contain only events valid against the events schema, in increasing
`sequence` order, and MUST end with exactly one `run_finished`.

- Kind: behavior
- Verify: test `mg_req_0260_json_output_is_valid_event_stream`

### MG-REQ-0261 · Nothing else on standard output

With `--json`, Mergus MUST NOT write anything to standard output that is not an event.

- Kind: behavior
- Verify: test `mg_req_0261_json_stdout_contains_only_events`

## 2. Exit codes

| Code | Meaning | Retryable |
|---|---|---|
| 0 | Success | — |
| 1 | Project error: configuration, model or contract | No |
| 2 | Quality blocked the publication | No, until data or test changes |
| 3 | Source or storage unavailable (transient) | Yes |
| 4 | Publication conflict: another writer moved the pointer | Yes, after a new plan |
| 5 | Menura rejected the catalog (schema gate) | No |
| 6 | Security: permission, secret, undecided PII | No |
| 7 | Cancelled by the user or by the time limit | Depends |
| 10–19 | Internal error of Mergus, with a report for an issue | No |

### MG-REQ-0262 · Exit codes have fixed meanings

Every command MUST end with one of the exit codes of this table, and its `run_finished`
event MUST carry the same code and a `retryable` flag as stated here.

- Kind: behavior
- Verify: test `mg_req_0262_exit_code_matches_table`

### MG-REQ-0263 · The CLI describes itself

`mergus schema <command>` MUST print in JSON the arguments, types, allowed values and exit
codes of a command; `mergus schema --all` MUST print it for the whole CLI.

- Kind: behavior
- Verify: test `mg_req_0263_schema_describes_every_command`

### MG-REQ-0264 · Actor

Every event MAY carry `actor`: a string set only by the host or by the authenticated CLI, that
names who started the command (a person or an agent acting for one). It MUST be used only for
audit and MUST NOT change permissions, plans or results. An actor written in a project file,
an argument or a request body MUST NOT be trusted or recorded as the actor.

- Kind: behavior
- Verify: test `mg_req_0264_actor_changes_nothing_but_audit`
- Decision: 0008
