---
id: governance-guards
title: What each CI guard checks
crate: none
phase: "0.1"
status: draft
depends_on: [governance-enforcement, governance-workflow]
---

# Guards

The guards of MG-REQ-1724 judge a pull request. These rules state their inputs and exactly
what makes each one fail, so that their result is not left to the implementer (decision
0003). "Implementation pull request" means one whose head branch starts with `issue-`; its
issue is the number in the branch name.

### MG-REQ-1730 · Guard inputs and output

Each guard MUST be a `cargo xtask <guard>` command that reads the pull request (number, base
and head commits, head branch, body) from the GitHub event file named by `GITHUB_EVENT_PATH`,
or from a file given with `--event <path>`, and reads anything else through the GitHub REST
API with the token in `GITHUB_TOKEN`. It MUST print one finding per line as
`<guard>: <message>` and exit with status 1 when it has a finding, 0 otherwise.

- Kind: process
- Verify: check `ci-guards`
- Decision: 0003

### MG-REQ-1731 · Scope guard

The `scope` guard MUST fail when:

1. MG-REQ-1731.1: the head branch matches none of `issue-<n>-<words>`, `spec/<words>` and
   `release/<major>.<minor>.<patch>`, where `<n>` is a positive integer without leading
   zero and `<words>` is lowercase letters and digits in words joined by single hyphens;
2. MG-REQ-1731.2: on an `issue-` branch, a changed file is not covered by the issue's
   "Files" section (a path ending in `/` covers everything under it), or is in the
   specification scope, or is an owner file of MG-REQ-1723 while the issue lacks the label
   `owner-authorized`;
3. MG-REQ-1731.3: on a `spec/` branch, a changed file is outside the specification scope;
4. MG-REQ-1731.4: on a `release/` branch, a changed file is not a `Cargo.toml`, a
   `pyproject.toml` or `CHANGELOG.md`;
5. MG-REQ-1731.5: in a Rust file (`*.rs`), an added `allow` or `expect` attribute, at item or
   crate level, has no written reason: neither a `reason = "<text>"` argument inside the
   attribute, which may span several lines, nor a `// reason: <text>` comment on the line
   before it; or an added `ignore` attribute marks a test (MG-REQ-0013). Other files are not
   checked by this item.

Only the owner adds the label `owner-authorized`, by hand. Sessions MUST NOT add it.

- Kind: process
- Verify: check `scope`
- Depends: MG-REQ-1707, MG-REQ-1723, MG-REQ-0013, MG-REQ-0014
- Decision: 0003

### MG-REQ-1732 · Commit-format guard

For every commit of the pull request that has one parent, the `commit-format` guard MUST
fail when:

1. MG-REQ-1732.1: the subject does not match
   `<type>(<scope>)!: <text>`, where `<type>` is one of `feat`, `fix`, `docs`, `style`,
   `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`, and `(<scope>)` and `!` are
   optional;
2. MG-REQ-1732.2: on an `issue-` branch, the subject does not end with `(#<n>)`, `<n>` being
   the branch's issue;
3. MG-REQ-1732.3: the message lacks a `Co-Authored-By:` trailer or a `Claude-Session:`
   trailer whose value is `https://claude.ai/code/session_` followed by letters and digits.

Merge commits (two parents) are not checked.

- Kind: process
- Verify: check `commit-format`
- Depends: MG-REQ-1708
- Decision: 0003

### MG-REQ-1733 · Session-identity guard

On an implementation pull request, the `session-identity` guard MUST fail when its
single-parent commits carry more than one `Claude-Session:` value, or when that value appears
in a commit of any other implementation pull request of the repository, open or closed. On
other branches it passes.

- Kind: process
- Verify: check `session-identity`
- Depends: MG-REQ-1702
- Decision: 0003

### MG-REQ-1734 · Independent-review guard

On an implementation pull request, the `independent-review` guard MUST pass only when the
latest pull request comment that contains a line `Verdict: <verdict>` says `ready`, was
created after the head commit's committer date, and carries a `Claude-Session:` line whose
value differs from every `Claude-Session:` value of the pull request's commits. On other
branches it passes; the owner reviews them.

- Kind: process
- Verify: check `independent-review`
- Depends: MG-REQ-1703
- Decision: 0003

### MG-REQ-1735 · Issue-format guard

On an implementation pull request, the `issue-format` guard MUST fail when the issue lacks
the label `impl`, or when its body lacks one of the headings `## Rules`, `## Crate`,
`## Read`, `## Acceptance criteria`, `## Out of scope`, `## Files`, `## Depends on`,
`## Parallel with`, or has one of them with no text, or names no `MG-REQ-` ID under
`## Rules`. On other branches it passes.

- Kind: process
- Verify: check `issue-format`
- Depends: MG-REQ-1704
- Decision: 0003

### MG-REQ-1736 · Pull-request-format guard

The `pr-format` guard MUST fail when the body lacks one of the headings of
`.github/PULL_REQUEST_TEMPLATE.md`, or has one of them with no text. On an implementation
pull request it MUST also fail when `## Issue` lacks `Closes #<n>` for the branch's issue, or
when `## Rules covered` does not name every `MG-REQ-` ID listed under the issue's
`## Rules`.

- Kind: process
- Verify: check `pr-format`
- Depends: MG-REQ-1706
- Decision: 0003

### MG-REQ-1737 · Deny-rules guard

The `deny-rules` guard MUST fail when `.claude/settings.json` at the head does not parse, or
when its `permissions.deny` list lacks an entry present in the same list at the base. The
base list is the reference: it changes only through a specification pull request the owner
merges.

- Kind: process
- Verify: check `deny-rules`
- Depends: MG-REQ-1701, MG-REQ-1721
- Decision: 0003

### MG-REQ-1738 · Repository-settings guard

The `repository-settings` guard MUST read the repository's rulesets and fail when:

1. MG-REQ-1738.1: no active branch ruleset targets the default branch with the rules
   `deletion`, `non_fast_forward` and `pull_request` and an empty bypass list;
2. MG-REQ-1738.2: no active tag ruleset targets `refs/tags/v*` with the rules `creation`,
   `update` and `deletion`, or its bypass list holds anything but the repository admin role;
3. MG-REQ-1738.3: the branch ruleset has a `required_status_checks` rule that lacks a
   blocking guard of MG-REQ-1724, or lists one without the GitHub Actions app
   (`integration_id` 15368).

When the branch ruleset has no `required_status_checks` rule, the guard prints a warning and
does not fail; the owner adds the rule once the guards run in CI (MG-REQ-1725).

- Kind: process
- Verify: check `repository-settings`
- Depends: MG-REQ-1720
- Decision: 0003

### MG-REQ-1739 · Release pull request check

On a `release/<version>` branch, the `pr-format` guard MUST also fail when:

1. MG-REQ-1739.1: the workspace version in the root `Cargo.toml` differs from `<version>`;
2. MG-REQ-1739.2: `CHANGELOG.md` has no `## <version>` section;
3. MG-REQ-1739.3: the body lacks a `## Gates` section naming every gate of section 2 of
   `roadmap.md`, each followed by a link to its evidence;
4. MG-REQ-1739.4: `cargo xtask traceability` reports a behavior rule of the released phase or
   an earlier one without a passing test.

- Kind: process
- Verify: check `release-gates`
- Depends: MG-REQ-1711, MG-REQ-1736
- Decision: 0009
