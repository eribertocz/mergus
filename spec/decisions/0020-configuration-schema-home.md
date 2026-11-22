# 0020 · Configuration schemas live in mergus-project

- Status: accepted
- Date: 2026-11-22
- Answers: spec-defect #95 and the question on #78 (blocked issue #78)

## Context

`schemas/` holds contract schemas only, and the `contracts-generated` check reads every
directory there as a contract, so `schemas/config/` failed it. A closed source schema with no
key would also reject every source file, including the one `mergus add source` writes.

## Decision

- The configuration schemas move to `crates/mergus-project/schemas/`; no check changes.
- Until phase 0.3, the source schema is open: any key, with TOML syntax, expressions
  (MG-REQ-0364) and literal secrets (MG-REQ-0320) still checked.
- `mergus` is checked as a string in phase 0.2; its syntax comes with MG-REQ-0319 (phase 0.9).

## Consequences

#78 needs no `xtask` change.
