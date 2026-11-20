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

cargo-deny:
    cargo deny check

# Pull request guards (MG-REQ-1730). They read the GitHub API, so `check` does not run them.
scope *args:
    cargo xtask scope {{args}}

commit-format *args:
    cargo xtask commit-format {{args}}

session-identity *args:
    cargo xtask session-identity {{args}}

# Every blocking check. Later issues add theirs.
check: fmt clippy unsafe missing-docs test boundaries language size secrets sql-concatenation crate-docs cargo-deny spec-check
