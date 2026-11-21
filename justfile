# Specification
spec-lint:
    python3 -I scripts/spec_tool.py lint

spec-index:
    python3 -I scripts/spec_tool.py index

spec-status:
    python3 -I scripts/spec_tool.py status

spec-check:
    python3 -I scripts/spec_tool.py check

spec-context +targets:
    python3 -I scripts/spec_tool.py context {{targets}}

spec-issue +rules:
    python3 -I scripts/spec_tool.py issue {{rules}}

# Rust
fmt:
    cargo fmt --all --check

clippy:
    cargo xtask clippy

unsafe:
    cargo xtask unsafe

missing-docs:
    cargo xtask missing-docs

test:
    cargo test --workspace --all-targets --locked

reproducible-build:
    cargo xtask reproducible-build

boundaries:
    cargo xtask check-boundaries

language:
    cargo xtask language

size:
    cargo xtask size

secrets:
    cargo xtask secrets

sql-concatenation:
    cargo xtask sql-concatenation

crate-docs:
    cargo xtask crate-docs

# Fails when a workflow grants a release write permission without being triggered only by a
# push of `v*` tags (MG-REQ-1715).
release-trigger:
    cargo xtask release-trigger

cargo-deny:
    cargo deny check

# Contract types (MG-REQ-0202): write them from `schemas/`, or fail when they are out of date.
contracts-generate:
    cargo xtask contracts-generate

contracts-generated:
    cargo xtask contracts-generated

# Pull request guards (MG-REQ-1730). They judge a pull request against its base, so `check`
# never runs them (MG-REQ-1724).
scope *args:
    cargo xtask scope {{args}}

commit-format *args:
    cargo xtask commit-format {{args}}

session-identity *args:
    cargo xtask session-identity {{args}}

independent-review *args:
    cargo xtask independent-review {{args}}

issue-format *args:
    cargo xtask issue-format {{args}}

pr-format *args:
    cargo xtask pr-format {{args}}

repository-settings *args:
    cargo xtask repository-settings {{args}}

# Compares each schema the pull request changes in place with its text at the base (MG-REQ-0205).
contract-compatibility *args:
    cargo xtask contract-compatibility {{args}}

# Needs no network: with `--base <revision>` it compares the working tree with that revision.
deny-rules *args:
    cargo xtask deny-rules {{args}}

# Traceability report (MG-REQ-0165). `--issue-rules <IDs>` also fails when a listed behavior
# rule has no passing test.
traceability *args:
    cargo xtask traceability {{args}}

# What waits for the owner: pull requests to merge, `needs-owner` issues, releases to tag
# (MG-REQ-1713). Reads the GitHub REST API with `GITHUB_TOKEN`.
owner-queue:
    cargo xtask owner-queue

# Every blocking check that needs no GitHub API, in order (MG-REQ-0166). `cargo xtask ci`
# holds the list, so `check` and CI cannot drift.
ci:
    cargo xtask ci

check:
    cargo xtask ci
