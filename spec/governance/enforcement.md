---
id: governance-enforcement
title: How the workflow is enforced
crate: none
phase: "0.1"
status: draft
depends_on: [governance-workflow]
---

# Enforcement

Sessions share the owner's GitHub account, so GitHub cannot tell them apart. The workflow is
enforced in three layers, each independent of an agent's goodwill.

| Layer | Mechanism | Stops |
|---|---|---|
| GitHub | Rulesets on `main` and on tags | Direct pushes, merges without green checks, tags by anything but the owner's hand |
| Claude Code | Committed `.claude/settings.json` deny rules | Merge, approve, tag, release, settings and raw API calls from sessions |
| CI | Guards run from the base commit | Out-of-scope changes, reused sessions, self-review, malformed issues and pull requests |

Limit, stated plainly: deny rules guard tools, they are not a sandbox. That is why
MG-REQ-0015 forbids workarounds and why the owner merges by hand.

### MG-REQ-1720 · Protected main

`main` MUST be protected by a ruleset requiring a pull request and every blocking check, and
forbidding force pushes and deletion. Tags `v*` MUST be protected so that only the owner can
create them.

- Kind: process
- Verify: check `repository-settings`

### MG-REQ-1721 · Deny rules

`.claude/settings.json` MUST deny, at tool level, the commands and API paths that merge,
approve, tag, release, push to `main`, force-push, delete branches, call GraphQL, or change
rulesets, secrets, variables, workflows, labels, collaborators or webhooks, including
GitHub MCP tools with those words in their names.

- Kind: process
- Verify: check `deny-rules`

### MG-REQ-1722 · Guards judge from the base commit

Every CI guard MUST run the guard scripts and read its configuration from the pull request's
base commit, and check out the pull request head only as data, so that a pull request cannot
change the rules it is judged by.

- Kind: process
- Verify: review `guards-from-base`

### MG-REQ-1723 · Specification scope

The specification scope is `spec/`, `AGENTS.md`, `CLAUDE.md`, `.claude/`,
`.github/ISSUE_TEMPLATE/`, `.github/PULL_REQUEST_TEMPLATE.md` and `scripts/spec_tool.py`.
A `spec/` branch MUST change only files inside it; an `issue-` branch MUST change none of
them; a `release/` branch MUST change only version files and `CHANGELOG.md`. The owner's
files (`.github/workflows/`, `deny.toml`, `CODEOWNERS`) change only by a pull request the
owner opens or explicitly authorizes.

- Kind: process
- Verify: check `scope`

### MG-REQ-1724 · Guard list

CI MUST run these guards on every pull request, all blocking except `language`:

| Guard | Checks |
|---|---|
| `scope` | MG-REQ-1707, MG-REQ-1723 |
| `session-identity` | MG-REQ-1702 |
| `independent-review` | MG-REQ-1703 (required before merge) |
| `issue-format` | MG-REQ-1704 |
| `pr-format` | MG-REQ-1706 |
| `commit-format` | MG-REQ-1708 |
| `spec-check` | Generated files up to date; `spec_tool lint` passes |
| `deny-rules` | MG-REQ-1701, MG-REQ-1721 |
| `repository-settings` | MG-REQ-1720 |
| `contract-compatibility` | MG-REQ-0204, MG-REQ-0205 |
| `language` | MG-REQ-0001 (warning only) |

A guard judges the pull request against its base, so it runs only in the guard workflow, from
the base commit (MG-REQ-1722). `cargo xtask ci` runs the checks that judge the tree alone and
never a guard (MG-REQ-0166).

- Kind: process
- Verify: check `ci-guards`
- Decision: 0005

### MG-REQ-1725 · Repository setup script

The specifier MUST provide `scripts/setup_repository.sh`, which the owner runs once to create
rulesets, tag protection, required checks and labels (`spec`, `impl`, `release`,
`spec-defect`, `needs-owner`, one label per crate).

- Kind: process
- Verify: review `setup-script`

### MG-REQ-1726 · Single-repository sessions

Sessions MUST run as single-repository Claude Code sessions started in this repository, so
that the committed `.claude/settings.json` applies from their first command.

- Kind: process
- Verify: review `session-type`
