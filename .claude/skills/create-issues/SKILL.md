---
name: create-issues
description: File the implementation issues of one roadmap phase from the specification. Use when asked to create issues for a phase.
---

# Create the issues of a phase

1. Read `spec/README.md`, `spec/roadmap.md` (the phase only) and `spec/status.md`.
2. List the phase's rules not yet `done`. Group them: one crate per issue, one to eight rules (MG-REQ-1705), dependencies first.
3. For each group, run `just spec-issue <IDs>` and file the issue with its output: milestone = phase, labels `impl` and the crate.
4. Order the issues. Fill "Depends on" with the issue before it, and "Parallel with" with `None`: issues are implemented one at a time (MG-REQ-1714).
5. Report the list of issues filed. Do not implement.
