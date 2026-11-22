# 0027 · Commands of phase 0.2

- Status: accepted
- Date: 2026-11-22
- Answers: open questions of #84 (changes MG-REQ-0260 and MG-REQ-0261)

## Context

#84 needed the `mergus schema` document, what `init` offers before storage profiles, what
`changed` and `explain` do before environments and engines, and the user configuration path.

## Decision

- `mergus schema` prints a contract document, `mergus_cli`. It is itself the machine-readable
  output, so it is the same with `--json` (an exception to MG-REQ-0260, which covers event
  streams of commands that act).
- `init` offers only `local`; the three next steps are fixed (MG-REQ-1349).
- Scaffolding commands get `--dry-run` (MG-REQ-0192).
- The only setting is `max_parallel`, the budget of the plan (MG-REQ-0744).
- `changed` compares with available results of `prod`, of which there are none in 0.2.
- The configuration path follows XDG on Unix and `%APPDATA%` on Windows.

## Consequences

`explain` and `init` grow with later phases; their 0.2 output is a subset.
