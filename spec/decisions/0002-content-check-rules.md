# 0002 · What the content checks enforce

- Status: accepted
- Date: 2026-11-20
- Answers: spec-defect #23 (blocked issue #7)

## Context

The checks `language`, `size`, `secrets`, `sql-concatenation` and `crate-docs` had only the
intent of their rules (MG-REQ-0001, MG-REQ-0012, MG-REQ-0016, MG-REQ-0007, MG-REQ-0111,
MG-REQ-0112). Their pass or fail result could not be implemented without guessing.

## Decision

Option A of #23 (one rule per check with exact scope and patterns), mixed with option C
(in-file waivers) where the rule itself allows a justified exception:

- MG-REQ-0176: common scope. Tracked files only; `fixtures` directories and the generated
  spec files are skipped; one output format.
- MG-REQ-0177: `language` warns on non-ASCII alphabetic characters and never fails.
- MG-REQ-0178: `size` measures Rust only and fails, unless the item carries a
  `// size: <reason>` waiver. MG-REQ-0012 is a SHOULD with a written justification, so the
  waiver is that justification, kept next to the code and checked in review. It is not an
  allowlist (MG-REQ-0013).
- MG-REQ-0179: `secrets` uses a short list of key and token patterns, file names, artifact
  and data extensions, and one heuristic for commented-out code. High-entropy detection was
  rejected for its false positives.
- MG-REQ-0184: `sql-concatenation` fails on formatted SQL with placeholders unless a waiver
  names the dialect's quoting function.
- MG-REQ-0115: `crate-docs` compares exact text and headings; `xtask` is exempt.

## Consequences

- #7 covers these rules.
- Generated spec files may exceed 400 lines; they are not Rust and are skipped.
- Every later crate issue must create `README.md` and `ARCHITECTURE.md` with the headings of
  MG-REQ-0115.
