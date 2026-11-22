# 0025 · Sources of the error lines, and verbosity

- Status: accepted
- Date: 2026-11-22
- Answers: open questions of #82

## Decision

- The generic "why" lives once, in the catalog file (MG-REQ-1346.3); the specific "fix" is the
  diagnostic's `hint`, so human and `--json` output say the same thing.
- The location has no end column in 0.2, so the underline is one caret (MG-REQ-1346.2). A
  wider underline needs an optional end column in the events contract later (minor change).
- `--quiet` and `--verbose` change only human output (MG-REQ-1347).
