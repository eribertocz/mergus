# 0005 · Guards run only from the base

- Status: accepted
- Date: 2026-11-20
- Answers: the `deny-rules` failure of the first CI run (PR #34)

## Context

`cargo xtask ci` ran `deny-rules --base main`. In CI the head checkout has no local `main`,
so the step failed. Fetching `main` in the CI job would hide the real problem: a check that
compares the pull request with its base is a guard, and a guard must run the base's code
(MG-REQ-1722), not the head's. `deny-rules` and `repository-settings` were also missing from
the guard list, so they were never required.

## Decision

MG-REQ-1724 lists `deny-rules` and `repository-settings` as blocking guards and states that
guards run only in the guard workflow. `cargo xtask ci` runs only checks that judge the tree
alone.

## Consequences

- #38 removes `deny-rules` from `cargo xtask ci` and adds both guards to the required lists
  (`repository-settings` guard, `scripts/setup_repository.sh`).
- #12 (PR #34) runs both from the base in `guards.yml` and drops the `main` fetch.
