# 0016 · One workspace version for every crate

- Status: accepted
- Date: 2026-11-21
- Answers: notes on release pull request #70 and handoff #71

## Context

Release 0.1.0 bumped only `[workspace.package]`; the member crates kept `version = "0.0.0"`.
Making them inherit the workspace version changes `Cargo.lock`, and MG-REQ-1731.4 did not let a
release branch touch it, so a release could not version the crates it ships.

## Decision

- Every member `Cargo.toml` inherits `version.workspace = true`. The `release-gates` extension
  of `pr-format` fails on a release branch when a member sets its own version (MG-REQ-1739.5).
- A release branch may change the root `Cargo.lock` (MG-REQ-1731.4), which the version bump
  regenerates.

## Consequences

One edit of the root `Cargo.toml` versions the whole workspace. The scope guard, the
`release-gates` extension and the current members change in one implementation issue before
release 0.2.0.
