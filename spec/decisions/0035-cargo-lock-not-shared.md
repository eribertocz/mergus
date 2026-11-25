# 0035 · The root Cargo.lock does not block parallel issues

- Status: accepted
- Date: 2026-11-25
- Answers: the owner's approval to run more issues in parallel

## Context

Almost every implementation issue of phase 0.3 creates a crate or adds a dependency, so it
changes the root `Cargo.lock`. Under MG-REQ-1714, that one shared file forced every issue into
a single chain, even when the issues touched different crates.

## Decision

The root `Cargo.lock` no longer counts as a file in common for MG-REQ-1714. Cargo generates
it, so a conflict there is resolved mechanically: merge `main`, regenerate the file with
Cargo, and pass CI again before merging. Issues that change the same crate or any other
common file still run one after another.

## Consequences

- More sessions run at the same time.
- A pull request that conflicts only in `Cargo.lock` needs one extra merge of `main` and one
  extra CI run before the owner merges it.
- Incompatible versions of a shared dependency surface in that second pull request, which
  resolves them.
