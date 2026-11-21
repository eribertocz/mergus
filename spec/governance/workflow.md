---
id: governance-workflow
title: Roles, issues, pull requests, reviews and releases
crate: none
phase: "0.1"
status: draft
depends_on: [kernel, governance-enforcement]
---

# Workflow

## 1. Roles

| Role | Actor | Does | Never |
|---|---|---|---|
| Owner | The human maintainer | Merges pull requests, creates release tags, answers `needs-owner` issues | — |
| Specifier | Claude Code session, label `spec` | Changes `spec/`, files issues from the roadmap, answers `spec-defect` issues, reviews implementation pull requests and posts a verdict | Writes implementation code, reviews a pull request it wrote code in, approves, merges, tags |
| Implementer | A new Claude Code session per issue, label `impl` | Implements one issue: branch, commits, one pull request | Edits `spec/`, reviews, merges, tags |
| Release preparer | Claude Code session, label `release` | Opens the release pull request | Tags, publishes |
| CI | GitHub Actions | Runs blocking checks; publishes after a tag | — |

All sessions act through the owner's GitHub account. Separation is enforced by the
mechanisms of `governance/enforcement.md`, not by goodwill.

### MG-REQ-1701 · Only the owner merges and tags

Merging into `main`, approving pull requests, creating tags, publishing releases and changing
repository settings MUST be done only by the owner.

- Kind: process
- Verify: check `deny-rules`

### MG-REQ-1702 · One session, one pull request

Every implementation pull request (branch `issue-…`) MUST be written by exactly one session
that has authored no other implementation pull request. All its commits MUST carry the same
`Claude-Session:` trailer, and that value MUST NOT appear in any other implementation pull
request of the repository. Specification and release pull requests are written by the
specifier or release preparer session, which MAY author several.

- Kind: process
- Verify: check `session-identity`
- Decision: 0003

### MG-REQ-1703 · Independent review

Every implementation pull request MUST receive a review comment from a session whose
`Claude-Session:` differs from the author's and from any session that wrote code in the same
pull request.

- Kind: process
- Verify: check `independent-review`

## 2. Life of a change

1. **Issues.** For a roadmap phase, the specifier runs `just spec-issue` for each group of
   rules and files the issues: milestone = phase, labels = `impl` plus crate, body with
   "Depends on" and "Parallel with".
2. **Implementation.** A new implementer session takes the next unblocked issue
   (`just next-issue`), creates `issue-<n>-<short-name>`, writes failing tests first, makes
   them pass, runs `just check`, opens the pull request.
3. **Review.** The specifier session runs the `review-pr` skill and posts one comment with
   findings by severity and a verdict: `ready`, `changes-needed` or `spec-defect`.
4. **Fixes.** On `changes-needed`, the same implementer session pushes new commits. It never
   force-pushes after review started.
5. **Merge.** The owner merges pull requests that have green CI and a `ready` verdict.
6. **Status.** `just spec-status` prints the state of every rule on demand.
7. **Release.** When every gate of a phase is green, the release preparer opens
   `release/<version>`. The owner merges it and pushes the tag `v<version>`. CI publishes.

### MG-REQ-1704 · Issue contents

Every implementation issue MUST contain: the rule IDs it covers, the crate, the "Read" list
produced by `just spec-issue`, acceptance criteria as test names, out of scope, the files it
may change ("Files"), "Depends on" and "Parallel with". An implementer that finds one missing MUST open a `spec-defect` issue
and stop.

- Kind: process
- Verify: check `issue-format`
- Decision: 0003

### MG-REQ-1705 · Issue size

An issue SHOULD cover between one and eight rules of one crate and be completable in one
session. Larger work is split into several issues.

- Kind: process
- Verify: review `issue-size`

### MG-REQ-1706 · Pull request contents

Every pull request MUST fill the template: issue number, rule IDs covered, the test proving
each rule, deviations raised as issues, and the self-check list.

- Kind: process
- Verify: check `pr-format`

### MG-REQ-1707 · Branch names

Branches MUST be named `issue-<n>-<short-name>`, `spec/<topic>` or `release/<version>`.

- Kind: process
- Verify: check `scope`

### MG-REQ-1708 · Commits

Commits MUST follow Conventional Commits with the issue number, and carry the
`Co-Authored-By` and `Claude-Session` trailers.

- Kind: process
- Verify: check `commit-format`

### MG-REQ-1714 · Parallel only without conflicts

Each issue MUST be implemented by a new session. Two issues MAY be implemented in parallel
only when they change no file in common, so that their pull requests cannot conflict; the
specifier lists them in each other's "Parallel with". Issues that change the same crate or the
same file MUST NOT be parallel: the later one names the earlier one in "Depends on". An
implementer session MUST NOT start an issue while an open implementation pull request belongs
to an issue that is not listed in its "Parallel with".

- Kind: process
- Verify: review `parallel-without-conflicts`

## 3. Doubts and escalation

| Doubt | Who decides | How |
|---|---|---|
| Invisible from outside the crate | Implementer | Decides and explains in the pull request |
| Observable behavior | Specifier | `spec-defect` issue → spec pull request (+ decision record if a rule changes meaning) |
| Owner-level | Owner | Issue labelled `needs-owner` |

### MG-REQ-1709 · What reaches the owner

The specifier MUST label an issue `needs-owner` only when the decision is irreversible,
changes a released public contract, affects security or privacy, touches licensing,
trademarks or cost, or contradicts an approved decision. The issue MUST state the options,
their consequences and a recommendation. Every other doubt is decided by the specifier.

- Kind: process
- Verify: review `escalation`

### MG-REQ-1710 · Spec defects block only their issue

An implementer that opens a `spec-defect` issue MUST stop work on its issue. Other issues
continue.

- Kind: process
- Verify: review `spec-defect-stop`

## 4. Releases

### MG-REQ-1711 · Release pull request

The release pull request MUST bump the version, contain the generated changelog, and show
that every gate of the phase in `roadmap.md` passes. It changes nothing else.

- Kind: process
- Verify: check `release-gates`

### MG-REQ-1712 · Tags trigger publishing

Only a tag `v<semver>` pushed by the owner on a commit of `main` MAY start the publishing
pipeline of `distribution/`.

- Kind: process
- Verify: check `release-trigger`

## 5. The owner's queue

`just owner-queue` lists, in this order: pull requests ready to merge, `needs-owner` issues,
and a release ready to tag. It is the only list the owner needs.

### MG-REQ-1713 · Owner queue

`just owner-queue` MUST list exactly the open pull requests with green CI and a `ready`
verdict, the open `needs-owner` issues, and any merged release pull request without a tag:

1. MG-REQ-1713.1: green CI means every required check of MG-REQ-1727 has a check run with
   conclusion `success` or `skipped` on the head commit, as GitHub's ruleset counts them;
2. MG-REQ-1713.2: a `ready` verdict is read as the `independent-review` guard reads it
   (MG-REQ-1734); a pull request outside that guard's branches needs none;
3. MG-REQ-1713.3: a release pull request without a tag is a merged pull request whose head
   branch is `release/<version>` and for which no tag `v<version>` exists;
4. MG-REQ-1713.4: each item is one line `<kind> #<number> <title> <url>`, with kind `merge`,
   `needs-owner` or `tag`, in that order of kinds and by ascending number within a kind. With no
   item, the command prints nothing and exits 0.

- Kind: process
- Verify: test `mg_req_1713_owner_queue_lists_ready_items`
- Decision: 0013

### MG-REQ-1715 · Release trigger check

The `release-trigger` check of `cargo xtask ci` MUST fail when a workflow under
`.github/workflows/` grants `contents: write`, `packages: write` or `id-token: write` and is
triggered by anything other than `push` of tags matching `v*`.

- Kind: process
- Verify: check `release-trigger`
- Depends: MG-REQ-1712
- Decision: 0009
