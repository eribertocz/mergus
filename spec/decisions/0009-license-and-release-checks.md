# 0009 · License, DCO, governance-files and release checks

- Status: accepted
- Date: 2026-11-21
- Prepares: issues #17 and #18, so that they do not stop on spec-defects

## Context

MG-REQ-1741, MG-REQ-1742, MG-REQ-1744, MG-REQ-1711 and MG-REQ-1712 named checks without exact
pass or fail criteria, as #20, #23 and #33 showed for earlier checks.

## Decision

- MG-REQ-1747: licenses are declared in each `Cargo.toml`, not as file headers.
- MG-REQ-1748: the DCO is part of the existing `commit-format` guard and applies only to
  pull requests from forks, so no new required check is needed.
- MG-REQ-1749: governance files must exist, be non-empty, and adopt the Contributor Covenant.
- MG-REQ-1739: release pull request criteria are part of the existing `pr-format` guard on
  `release/` branches.
- MG-REQ-1715: only workflows triggered by `v*` tags may hold write or OIDC permissions.

Checks of the tree alone run in `cargo xtask ci`; checks of a pull request extend existing
guards (decision 0005), so the list of required checks (MG-REQ-1727) does not change.
