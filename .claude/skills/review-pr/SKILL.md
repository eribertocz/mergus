---
name: review-pr
description: Review one Mergus pull request against its issue and the specification, and post a verdict. Use when asked to review a pull request.
---

# Review a pull request

1. Confirm this session is the specifier session and wrote no commit of this pull request (MG-REQ-1703).
2. Read the pull request, its files and its issue with the GitHub MCP tools
   (`mcp__github__pull_request_read`, `mcp__github__issue_read`); `gh` is only for search.
3. Read `spec/kernel.md` and the output of `just spec-context <issue>`. Open a rule's source file under `spec/` only when the context is not enough.
4. Check and record each finding with file and line:
   - Traceability: every rule of the issue has a passing test named after it; no test claims a rule the code does not implement.
   - Scope: only the issue's crate and files changed.
   - Behavior: nothing the rules do not define; deviations raised as issues.
   - Kernel: MG-REQ-0001 to MG-REQ-0016.
   - AI-native: a new command has `--json`, a `mergus schema` entry, `--dry-run` and an idempotency key when it changes state, stable identifiers and actionable diagnostics (MG-REQ-0192).
   - No AI inside: no language model, AI SDK or AI provider dependency or call (MG-REQ-0190); agent changes arrive only as reviewed branches (MG-REQ-0191).
   - Portability: paths handled as path types, printed with `/`; CI green on Linux, macOS and Windows (MG-REQ-0189).
   - Review items: for every rule of the issue verified by `review <item>`, the diff meets the rule's statement.
   - Parallel work: the issue's "Depends on" is closed, and no open implementation pull request outside its "Parallel with" changes the same files (MG-REQ-1714).
   - Determinism, security and bound parameters where the change touches them.
5. Confirm CI is green: `mcp__github__pull_request_read` with `get_check_runs`.
6. Post one comment with findings by severity (blocking, should fix, note), the line `Verdict: ready | changes-needed | spec-defect`, and the trailer `Claude-Session: <id>`.
7. Never approve, never merge.

Keep findings short and concrete: file, line, problem, fix.
