---
name: implement-issue
description: Implement one Mergus issue labelled impl, from reading it to opening its pull request. Use when asked to implement or work on an issue.
---

# Implement an issue

Use the GitHub REST API through `gh api`; GraphQL is not available.

1. Confirm this session has authored no pull request. If it has, stop: one session, one pull request (MG-REQ-1702).
2. Read the issue: `gh api repos/{owner}/{repo}/issues/<n>`. Check it has every section of MG-REQ-1704. If one is missing, open a `spec-defect` issue and stop.
3. Check every issue under "Depends on" is closed, and every open implementation pull request belongs to an issue listed under "Parallel with" (MG-REQ-1714). If not, stop and name it.
4. Read `spec/kernel.md`, then run `just spec-context <n>` and read its output. Read nothing else under `spec/`.
5. `git switch -c issue-<n>-<short-name>` from an up-to-date `main`.
6. For each behavior rule, write the test `mg_req_<id>_<what>` first, run it, see it fail.
7. Write the smallest code that makes the tests pass, inside the issue's crate only.
8. Run `just check`. Fix everything; never weaken a check.
9. Review your diff: scope, no `unwrap`, no commented-out code, public items documented.
10. Commit (Conventional Commits, issue number, trailers `Co-Authored-By` and `Claude-Session`). Push.
11. Open the pull request with the template filled, then add label `impl`.
12. Stop. Do not review, merge or tag.
