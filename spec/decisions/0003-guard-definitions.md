# 0003 · What the CI guards check

- Status: accepted
- Date: 2026-11-20
- Answers: the gaps found while preparing issues #8, #9 and #10, after spec-defects #20
  and #23 showed that checks without exact criteria block implementers.

## Context

The guards of MG-REQ-1724 and the `cargo-deny` check had only the intent of their rules.
Two rules also no longer matched how the project works:

- MG-REQ-1702 forbade a session from authoring more than one pull request. The owner
  decided that one long-lived specifier session writes specification pull requests and
  reviews, so the rule must apply to implementation pull requests only.
- MG-REQ-1704 did not list the files an issue may change, but MG-REQ-0014 and the `scope`
  guard need that list.

## Decision

- MG-REQ-1702 applies to implementation pull requests (`issue-` branches).
- MG-REQ-1704 adds a "Files" section; `spec_tool.py issue` and the issue template print it.
- `governance/guards.md` defines each guard's inputs, output and failures (MG-REQ-1730 to
  MG-REQ-1738).
- Owner authorization for owner files is the label `owner-authorized`, added by the owner
  by hand. Sessions share the owner's GitHub account, so this cannot be enforced
  mechanically; it relies on MG-REQ-0015 and review, like the other limits of
  `governance/enforcement.md`.
- MG-REQ-1746 fixes the contents of `deny.toml`, including the OSI-approved license list.

## Consequences

- Open issues #7 to #18 get a "Files" section.
- Issues #8 and #12 are started once the owner adds `owner-authorized`.
- The specifier session's specification pull requests pass `session-identity`.
