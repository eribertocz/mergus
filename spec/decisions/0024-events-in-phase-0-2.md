# 0024 · Events contract, invalid command line and actor in phase 0.2

- Status: accepted
- Date: 2026-11-22
- Answers: open questions of #81

## Context

#81 needed the events schema home, the formats of the common fields, the exit code of an
invalid command line, how `actor` arrives, and what `mergus validate` runs.

## Decision

- The contract is `mergus_events`, following `schemas/README.md`. Its schema keywords cannot
  express per-type requirements, so the code checks them (MG-REQ-0295).
- `run_id` is a W3C trace id so it can be the OpenTelemetry trace id (MG-REQ-1101).
- An invalid command line is exit code 1: it is a user error that retrying does not fix.
  Clap's usual 2 would collide with "quality blocked".
- No `actor` until a host interface or CLI authentication exists; none does in 0.2.
- `validate` is stage 1; the project is found by walking up to `project.toml`.

## Consequences

The meaning of exit code 1 widens before the first release; no released contract changes.
