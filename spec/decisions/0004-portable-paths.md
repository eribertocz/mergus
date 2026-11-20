# 0004 · Portable paths

- Status: accepted
- Date: 2026-11-20
- Answers: the Windows failures of `xtask` found by the first CI run (PR #34, issue #38)

## Context

Until CI ran on three systems, every check was written and tested on Linux only. On Windows
two tests failed because findings printed `\` and paths were compared as strings with `/`.
No rule said how paths are printed or compared.

## Decision

MG-REQ-0189: path types and component comparison in code; `/` and workspace-relative paths in
everything printed; tests that touch paths pass on the three systems of MG-REQ-0166. Reviews
check it; CI on Linux, macOS and Windows enforces it once the required checks are on.

## Consequences

#38 fixes every existing `xtask` check. Later issues follow the rule from the start.
