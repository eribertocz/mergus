# Changelog

All notable changes to Mergus. Generated from the commit messages of `main`.

## 0.1.0 - 2026-11-21

Phase 0.1, Foundations: repository, CI guards, specification tool, ports, simulator
skeleton and contract schemas.

### Features

- feat(scripts): add repository setup script (#1)
- feat(xtask): add the scope, commit-format and session-identity guards (#9)
- feat(xtask): add the independent-review, issue-format, pr-format, deny-rules and repository-settings guards (#10)
- feat(xtask): add the traceability report and `cargo xtask ci` (#11)
- feat(ports): add the ports and their simulated implementations (#13)
- feat(xtask): accept a reason argument in the scope guard (#46)
- feat(sim): add seeded runs, replay, shrinking and accelerated time (#14)
- feat(contracts): add schema layout, generated types and validation before emitting (#15)
- feat(contracts): add the compatibility guard, minor-version reads and canonical JSON (#16)
- feat(xtask): add the license-headers and governance-files checks (#17)
- feat(xtask): check the DCO sign-off of fork commits in commit-format (#17)
- feat(xtask): add release-gates, release-trigger and owner-queue (#18)
- feat(xtask): check contract VERSION files against schema changes (#59)

### Fixes

- fix(scripts): pin required checks to GitHub Actions (#1)
- fix(xtask): write paths in findings with `/` on every system (#38)
- fix(xtask): handle paths by components and print them with `/` (#38)
- fix(xtask): run no guard in `cargo xtask ci` and require nine guards (#38)
- fix(spec): sort specification files the same way on every system
- fix(xtask): count skipped required checks as green in owner-queue (#18)
- fix(xtask): drop spec/status.md from the generated-file lists (#60)

### Refactoring

- refactor(xtask): split `type_of` of the contract generator under 50 lines (#15)

### Tests

- test(sim): split the scenario helpers to stay under 50 lines (#13)

### Build

- build: add workspace, pinned toolchain, lints, xtask and `just check` (#5)
- build: add `cargo xtask check-boundaries` to `just check` (#6)
- build: add the content checks to `just check` (#7)
- build: add the cargo-deny dependency policy to `just check` (#8)
- build: check out every text file with LF line endings (#15)

### Continuous integration

- ci: add the guard workflow from the base commit and the CI workflow (#12)
- ci: re-run the head's independent-review check on pull request comments (#12)
- ci: fetch main before `cargo xtask ci` (#12)
- ci: add the deny-rules and repository-settings guards (#12)
- ci: require the checks of MG-REQ-1727 and drop tree checks from the guards (#46)
- ci: wait for the guards run to complete before re-running independent-review (#52)
- ci: run the contract-compatibility guard in the scope job (#52)

### Documentation

- docs(spec): initial Mergus specification, workflow and agent setup
- docs(spec): the specifier session reviews implementation pull requests
- docs(spec): implement issues one at a time (MG-REQ-1714)
- docs(spec): allow parallel issues that change no file in common (MG-REQ-1714)
- docs(spec): give implementers the explanatory text of their rules' files
- docs(spec): state what the boundary check enforces (#20)
- docs(spec): define what the content checks enforce (#23)
- docs(spec): define the CI guards and deny.toml (decision 0003)
- docs: add license, pledge and governance documents (#26)
- docs: adopt the Contributor Covenant 2.1 by reference (#26)
- docs(spec): report implementation status on GitHub, not in the chat
- docs(spec): define the scope of the traceability report (#33)
- docs(spec): require portable paths (MG-REQ-0189)
- docs(spec): guards run only from the base (decision 0005)
- docs(spec): contract-compatibility is a guard (decision 0005)
- docs(spec): print the rule status on demand instead of storing it (#43)
- docs(spec): checks of the tree alone run in CI, not as guards (decision 0007)
- docs(spec): keep spec/status.md until the spec-check guard is gone (#43)
- docs(spec): regenerate spec/status.md for MG-REQ-1727
- docs(spec): accept Rust's `reason = "…"` as the written reason (MG-REQ-1731.5)
- docs(spec): MG-REQ-1731.5 checks Rust files only
- docs(spec): AI-ready with no AI inside (decision 0008)
- docs(spec): review items for MG-REQ-0190 and MG-REQ-0191
- docs(spec): exact criteria for license, DCO, governance and release checks (decision 0009)
- docs(spec): agents are first-class users (decision 0010)
- docs(spec): AI-native architecture principle and guarantees (decision 0010)
- docs(spec): agents best in class (decision 0011)
- docs(spec): run contract-compatibility inside the scope job
- docs(spec): contract version format; implementers write nothing in chat
- docs(spec): define release gate lines and the owner queue output (#63)
- docs(spec): contracts Mimus needs from Mergus; one MCP server per module
- docs(spec): a skipped required check counts as green in the owner queue
- docs(spec): MCP server inside the repository, actor kind, review report

### Reverts

- revert: stop fetching main in the ci workflow (#12)
