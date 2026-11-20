# 0007 · Checks of the tree alone run in CI, not as guards

- Status: accepted
- Date: 2026-11-20
- Answers: the `spec-check` failure of PR #44, the first pull request judged by the guards

## Context

`spec-check` ran as a guard: the base's `spec_tool.py` judged the head's tree. A pull request
that changes `spec_tool.py` itself (#44 changed which files it generates) could never pass,
because the old tool judged the new layout. `spec-check` and `language` judge the tree alone,
not the pull request against its base, and `cargo xtask ci` already runs both with the head's
code on three systems. Running the head's tool is safe there: `ci.yml` has no secrets, and the
`scope` guard keeps `scripts/spec_tool.py` in the specification scope that only the owner merges.

## Decision

- `spec-check` and `language` leave the guard list of MG-REQ-1724 and stay in `cargo xtask ci`.
- MG-REQ-1727 lists the required checks: the guards that have a job, `ci` on the three systems,
  `traceability` and `benchmarks`. A guard becomes required in the change that adds its job,
  so a missing job can never block every pull request.

## Consequences

An issue removes the `spec-check` and `language` jobs from `guards.yml` and updates the required
lists (`repository-settings` guard, `scripts/setup_repository.sh`). Then the owner turns on the
required checks.
