---
id: architecture-boundaries
title: Dependency direction and ownership boundaries
crate: none
phase: "0.1"
status: draft
depends_on: [architecture-crates]
---

# Boundaries

Layers come from `architecture/crates.md`. A crate depends only on crates of a lower layer.

```
0  contracts, ports
1  project
2  plan
3  engines, sources, adapters, catalog, quality, ops, security, menura, fetch
4  runtime
5  cli, py
```

`cargo xtask check-boundaries` reads `cargo metadata` and fails on any violation below.

### MG-REQ-0120 · Downward dependencies only

A crate MUST depend only on crates of a strictly lower layer, except that layer-3 crates MAY
depend on `mergus-security` and `mergus-ops`. Cycles MUST NOT exist.

- Kind: process
- Verify: check `boundaries`

### MG-REQ-0121 · Planning is pure

`mergus-plan` MUST NOT depend on engines, sources, adapters, `mergus-runtime` or the I/O
implementations of `mergus-ports`.

- Kind: process
- Verify: check `boundaries`

### MG-REQ-0122 · Single writers

Only adapter crates MUST write to storage. Only `mergus-catalog` MUST write catalog
documents and move the environment pointer.

- Kind: behavior
- Verify: check `boundaries`

### MG-REQ-0123 · Secrets have one door

Only `mergus-security` MUST resolve secret references into values. Other crates receive
connection handles, never secret values.

- Kind: behavior
- Verify: check `boundaries`

### MG-REQ-0124 · Test crates stay in tests

No crate MUST depend on `mergus-sim` or `mergus-testkit` except under `[dev-dependencies]`.

- Kind: process
- Verify: check `boundaries`

### MG-REQ-0125 · External dependencies are approved

A new external crate MUST be maintained, widely used, license-compatible and allowed in
`deny.toml`, which only the owner changes (MG-REQ-1723).

- Kind: process
- Verify: check `cargo-deny`
