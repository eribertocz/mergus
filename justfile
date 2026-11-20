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

# Every blocking check. Later issues add theirs.
check: fmt clippy unsafe missing-docs test spec-check
