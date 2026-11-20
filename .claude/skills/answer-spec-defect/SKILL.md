---
name: answer-spec-defect
description: Resolve a spec-defect issue by changing the specification. Use when asked to answer or fix a specification defect.
---

# Answer a specification defect

1. Read the defect, `spec/README.md`, `spec/MAP.md` and the files of the rules it names.
2. Decide. If the decision meets MG-REQ-1709, open a `needs-owner` issue with options and a recommendation, and stop.
3. Otherwise, on `spec/<topic>`, change or add rules (never renumber; retire instead). If a rule changes meaning, add `spec/decisions/NNNN-<title>.md`.
4. Run `just spec-lint` and `just spec-index`. Open the pull request linking the defect.
5. Comment on the defect with the rule IDs that now answer it.
