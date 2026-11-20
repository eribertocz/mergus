# 0006 · Rule status is printed on demand

- Status: accepted
- Date: 2026-11-20
- Answers: spec-defect #43 (blocked issue #13)

## Context

`spec/status.md` was generated from the rules and from the test names found in the code, and
`spec-check` failed when it was out of date. Every new `mg_req_` test therefore changed a file
in the specification scope, which implementation branches may not touch (MG-REQ-1723). No
implementation pull request with a new rule test could pass.

## Decision

The status depends on the code, so it is not stored. `spec_tool.py status` (`just spec-status`)
prints it on demand. `spec/status.md` is removed, and `spec-check` compares only `RULES.md` and
`crates.md`, which depend on the specification alone.

## Consequences

The specifier and the release preparer run `just spec-status` instead of reading a file.
- Transition: until the `spec-check` guard leaves `guards.yml` (#46, decision 0007), the base's
  tool still checks `spec/status.md`, so the file stays, unchanged, until then. The
  specification pull request after #46 deletes it.
