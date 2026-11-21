# 0017 · Closest names and the error catalog check

- Status: accepted
- Date: 2026-11-21
- Builds on: phase 0.2 issue filing

## Context

Phase 0.2 rules ask for "the closest valid key", "the closest known name" and "the closest
valid suggestion" without saying how they are chosen, and MG-REQ-1331 to MG-REQ-1333 name an
`error-catalog` check without its exact findings or the file format of the catalog. Both are
observable, so implementers would have to guess (MG-REQ-0002).

## Decision

- Closest known name: smallest Levenshtein distance, ties by byte order, only within
  `max(2, n / 3)` (MG-REQ-0359).
- Invalid names get a mechanical suggestion: lowercase, runs of other characters become `_`,
  trimmed, `n_` before a leading digit, cut to 63 (MG-REQ-0360).
- The catalog file format is shown in `cli/errors.md`; MG-REQ-1335 lists the findings of the
  check. Codes of an area run without gaps, which also catches a removed code.

## Consequences

The first issues of phase 0.2 can be implemented from their context alone. The check reads
only files of the tree, so it runs in `cargo xtask ci` like the other tree checks.
