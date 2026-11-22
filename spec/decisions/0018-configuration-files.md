# 0018 · Configuration schemas, literal secrets and the version pin

- Status: accepted
- Date: 2026-11-22
- Answers: open questions of issue #78; note on pull request #85

## Context

Issue #78 could not be implemented from its context: which keys exist in phase 0.2, where the
configuration schemas live and which keywords they may use (the contract subset has no way to
say "string or `false`"), what a literal secret is, and how TOML dates map to canonical JSON.
MG-REQ-0319 needs the version download of `distribution/`, due in phase 0.9. MG-REQ-1739.5
let a member without any `version` key pass, although Cargo then gives it `0.0.0`.

## Decision

- Configuration schemas live in `schemas/config/`, are not contract documents and may also use
  type arrays, `oneOf`, `pattern`, `minimum`, `maximum` and `default` (MG-REQ-0361).
- A key enters a schema only when its rule is due; phase 0.2 knows `name`, `mergus`, `[gold]`,
  `[raw]`, `[ops]` and the folder options. Source files get keys in phase 0.3 (MG-REQ-0362).
- Literal secrets: a URL with a password, a private key block, or a secret-like key name with
  a value that is not `secret:` (MG-REQ-0363). Exit code 6, value never shown.
- `${`, `{{` and `{%` in a string are expressions (MG-REQ-0364).
- TOML maps to JSON as listed in MG-REQ-0365, dates as RFC 3339 strings.
- MG-REQ-0319 moves to phase 0.9, with the version manager it relies on.
- MG-REQ-1739.5 also fails on a member that sets no version.

## Consequences

Issue #78 can start once this is merged; it adds MG-REQ-0361 to MG-REQ-0365. A small
`xtask` issue updates MG-REQ-1739.5.
