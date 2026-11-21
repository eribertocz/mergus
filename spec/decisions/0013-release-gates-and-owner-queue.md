# 0013 · Release gates and owner queue formats

- Status: accepted
- Date: 2026-11-21
- Answers: spec-defect #63 (blocked issue #18)

## Context

MG-REQ-1739 named the gates of roadmap section 2 without their format or the form of the
evidence, and asked a guard to run `cargo xtask traceability`, which would run head code from a
guard (MG-REQ-1722). MG-REQ-1713 did not define green CI, the verdict reading, the release
item or the output.

## Decision

- Gates are the first column of the table in roadmap section 2, read from the base. Each needs
  a body list item with the gate's name, a colon and an inline Markdown link to an
  `https://` URL. All gates apply to every phase.
- `release-gates` names the `pr-format` extension; no new command or required check.
- MG-REQ-1739.4 is dropped: the required `traceability` check already enforces it on the release
  pull request, where the released phase is the current phase. No check is weakened.
- Owner queue: green is every required check of MG-REQ-1727 with `success`; the verdict is read
  as `independent-review` reads it; a release item is a merged `release/<version>` pull request
  without tag `v<version>`; one line per item, `merge`, `needs-owner`, `tag`.

## Consequences

#18 can be implemented from the context alone.
