---
id: architecture-crates
title: Workspace layout and crate responsibilities
crate: none
phase: "0.1"
status: draft
depends_on: [architecture-overview]
---

# Crates

## 1. Repository layout

```
mergus/
  Cargo.toml            workspace, members = ["crates/*"]
  rust-toolchain.toml   pinned toolchain, edition 2024
  crates/               one directory per crate, flat (no nesting)
  schemas/              public JSON Schemas of every contract
  python/               the thin `mergus` Python package
  tests/                end-to-end, sqllogictest (.slt), differential suites
  bench/                benchmarks
  docs/                 user documentation
  spec/                 this specification
  xtask/                repository automation (boundaries, traceability, ci)
  ui/                   web UI (post-MVP)
```

## 2. Crates

`Core` crates are pure: no I/O, clock, network, randomness, processes or environment
(MG-REQ-0004). Layer numbers define the dependency direction of `architecture/boundaries.md`.

| Crate | Layer | Core | Responsibility (one) |
|---|---|---|---|
| `mergus-contracts` | 0 | yes | Rust types generated from `schemas/` |
| `mergus-ports` | 0 | no | I/O traits and their real implementations |
| `mergus-project` | 1 | yes | Read and validate the project: files, `project.toml`, folder `mergus.toml` |
| `mergus-plan` | 2 | yes | Resolve, order and fingerprint the work; produce the plan |
| `mergus-engine-duckdb` | 3 | no | Execute DuckDB steps |
| `mergus-engine-polars` | 3 | no | Execute Polars steps |
| `mergus-source` | 3 | no | Extract from databases (ADBC) and files |
| `mergus-source-rest` | 3 | no | Extract from REST APIs declared in TOML |
| `mergus-adapter` | 3 | no | Storage adapter contract (public API for third parties) |
| `mergus-adapter-ducklake` | 3 | no | DuckLake storage adapter |
| `mergus-catalog` | 3 | no | Catalog documents, manifests, pointer compare-and-swap |
| `mergus-quality` | 3 | no | Tests, contracts, monitors, gate decisions |
| `mergus-ops` | 3 | no | Journal, ops tables, logs, OpenTelemetry, OpenLineage |
| `mergus-security` | 3 | no | Secret resolution, PII handling, hash chains |
| `mergus-menura` | 3 | no | Bridge to Menura contracts |
| `mergus-fetch` | 3 | no | Download, verify and cache versions and adapters |
| `mergus-runtime` | 4 | no | Run cycle and scheduling; wires ports to crates |
| `mergus-cli` | 5 | no | The `mergus` binary; translates arguments and output only |
| `mergus-py` | 5 | no | Python bindings (maturin) |
| `mergus-certify` | 3 | no | Conformance kit every storage adapter must pass |
| `mergus-sim` | test | no | Deterministic simulator |
| `mergus-testkit` | test | no | Fixtures, generators, golden datasets |

### MG-REQ-0110 · Flat workspace

Every crate MUST live in `crates/<crate-name>/` and be a member of the root workspace. Crates
MUST NOT be nested.

- Kind: process
- Verify: check `boundaries`

### MG-REQ-0111 · One responsibility per crate

Each crate MUST have the single responsibility stated in this table, written as the first
line of its `lib.rs` documentation and in its `README.md`. A new crate needs a specification
change adding it here.

- Kind: process
- Verify: check `crate-docs`

### MG-REQ-0112 · Crate documentation

Each crate MUST contain `README.md` (what it does, what it does not) and `ARCHITECTURE.md`
(main types, data flow, dependencies), each under 150 lines.

- Kind: process
- Verify: check `crate-docs`

### MG-REQ-0113 · Pinned toolchain

The repository MUST pin the Rust toolchain in `rust-toolchain.toml`, use edition 2024, and
build reproducibly: two builds of the same commit produce identical binaries.

- Kind: process
- Verify: check `reproducible-build`

### MG-REQ-0114 · Business logic stays out of the edges

`mergus-cli` and `mergus-py` MUST contain no business logic: they parse input, call
`mergus-runtime` or library crates, and render output.

- Kind: process
- Verify: review `thin-edges`
