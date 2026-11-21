# 0012 · Contract version format

- Status: accepted
- Date: 2026-11-21
- Answers: open questions on pull request #57 (issue #16)

## Context

MG-REQ-0204 gave each contract a semantic version but did not say where the minor version
lives. #16 stored only the major (`v<major>` folder, integer `contract_version`), so nothing
recorded a minor or patch change. The rule also did not say whether loosening changes are
minor; the review of #57 accepted them as major. It also asked which hash algorithm
`mergus-contracts` fixes.

## Decision

- The full version `MAJOR.MINOR.PATCH` is in `schemas/<contract>/v<MAJOR>/VERSION` and is
  exported by `mergus-contracts` as a constant.
- Patch: `title` or `description` only. Minor: an added optional property or `$defs` entry.
  Major: everything else, loosening included.
- Documents keep the integer major in `contract_version`. Minor changes add only optional
  fields, so a reader of the latest minor reads every older minor (MG-REQ-0206). The schema
  keyword set has no `pattern`, so a version string could not be validated anyway.
- `contract-compatibility` requires the `VERSION` increment that matches the change.
- Hashing: the algorithm belongs to each consumer rule (SHA-256 in `run/plan.md` and
  `contracts/manifest.md`). `mergus-contracts` returns the canonical bytes only.

## Consequences

A follow-up implementation issue adds the `VERSION` files, the constant and the guard check.
`spec/status.md` is removed in the same change (decision 0006).
