# 0001 · What the boundary check can enforce

- Status: accepted
- Date: 2026-11-20
- Answers: spec-defect #20 (blocked issue #6)

## Context

`architecture/boundaries.md` said that `cargo xtask check-boundaries` reads `cargo metadata`
and fails on any violation of its rules. Metadata shows packages, features and dependency
edges, not what code does. Several rules (MG-REQ-0003, MG-REQ-0004, MG-REQ-0121,
MG-REQ-0122, MG-REQ-0123) could not be turned into a pass or fail result without guessing.

## Decision

Split verification (option C of #20), with the metadata part stated as rules (option A):

- MG-REQ-0126 lists exactly what the boundary check fails on, using only `cargo metadata`.
- MG-REQ-0133 puts the real implementations of `mergus-ports` behind the non-default
  feature `real`, so purity (MG-REQ-0004, MG-REQ-0121) is visible in metadata: core crates
  and `mergus-plan` may use the port traits but never enable `real`.
- Direct I/O in code stays with Clippy (MG-REQ-0130).
- MG-REQ-0122 (single writers) and MG-REQ-0123 (secrets door) keep their meaning but are
  verified by tests in the phases that create the writers (0.5) and secret resolution (0.3).
  The check adds one metadata fact for MG-REQ-0123: only `mergus-security` depends on
  `keyring`.
- Scope: `xtask` has no layer; `mergus-sim` and `mergus-testkit` may depend on layers 0 to
  4; dev-dependencies are ignored for layers and cycles; a workspace crate missing from the
  crate table fails.

A source scan (option B) was rejected: text scans are easy to evade, give false positives,
and duplicate MG-REQ-0130.

## Consequences

- #6 covers MG-REQ-0003, MG-REQ-0004, MG-REQ-0110, MG-REQ-0120, MG-REQ-0121, MG-REQ-0124
  and MG-REQ-0126. MG-REQ-0122 and MG-REQ-0123 move to phases 0.5 and 0.3.
- #13 (ports) adds MG-REQ-0133.
