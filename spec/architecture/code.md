---
id: architecture-code
title: Code rules beyond the kernel
crate: none
phase: "0.1"
status: draft
depends_on: [kernel]
---

# Code rules

The kernel already forbids panics, `unsafe` in general, untyped errors and oversized
functions. This file adds the rules specific to how crates are written.

### MG-REQ-0170 · Unsafe only in FFI crates

`#![forbid(unsafe_code)]` MUST be set in every crate except those binding foreign code
(DuckDB, Polars, Python). There, each `unsafe` block MUST carry a `// SAFETY:` comment and a
test.

- Kind: process
- Verify: check `unsafe`

### MG-REQ-0171 · Error types

Library crates MUST define errors with `thiserror`. `anyhow` MAY be used only in
`mergus-cli` and `xtask`. Invariant assertions are the only allowed panics, and they are
listed in `architecture/invariants.md`.

- Kind: process
- Verify: check `clippy`

### MG-REQ-0172 · Logging through ops

Logs and traces MUST use `tracing` and be configured only by `mergus-ops`. Crates MUST NOT
print to standard output or standard error, except `mergus-cli`.

- Kind: process
- Verify: check `clippy`

### MG-REQ-0173 · Names state intent

Names MUST be full English words that state intent. Abbreviations other than widely known
ones (`id`, `sql`, `url`, `io`) MUST NOT be used.

- Kind: process
- Verify: review `naming`

### MG-REQ-0174 · Invalid states unrepresentable

Domain values MUST use enums and newtypes rather than strings and boolean flags when the set
of valid values is known.

- Kind: process
- Verify: review `types`

### MG-REQ-0175 · Public items documented

Every public item MUST have documentation stating what it does and its errors, with an
example where useful.

- Kind: process
- Verify: check `missing-docs`
