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

`cargo xtask check-boundaries` reads `cargo metadata` and fails on the violations listed in
MG-REQ-0126. What metadata cannot show is verified elsewhere: direct I/O outside
`mergus-ports` by Clippy (MG-REQ-0130), single writers and the secrets door by tests.

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
- Verify: test `mg_req_0122_only_adapters_and_catalog_write`
- Phase: 0.5
- Decision: 0001

### MG-REQ-0123 · Secrets have one door

Only `mergus-security` MUST resolve secret references into values. Other crates receive
connection handles, never secret values.

- Kind: behavior
- Verify: test `mg_req_0123_only_security_resolves_secrets`
- Phase: 0.3
- Decision: 0001

### MG-REQ-0124 · Test crates stay in tests

No crate MUST depend on `mergus-sim` or `mergus-testkit` except under `[dev-dependencies]`.

- Kind: process
- Verify: check `boundaries`

### MG-REQ-0125 · External dependencies are approved

A new external crate MUST be maintained, widely used, license-compatible and allowed in
`deny.toml`, which only the owner changes (MG-REQ-1723).

- Kind: process
- Verify: check `cargo-deny`

### MG-REQ-0126 · What the boundary check enforces

`cargo xtask check-boundaries` MUST use only `cargo metadata` and MUST fail when any of the
following holds. Workspace members are the crates of the crate table in
`architecture/crates.md`, plus `xtask`.

1. MG-REQ-0126.1: a member other than `xtask` is not in `crates/<its name>/`, or is not in
   the crate table. `xtask` lives in `xtask/` and has no layer.
2. MG-REQ-0126.2: a normal or build dependency between members breaks MG-REQ-0120, using the
   layers of the crate table. `mergus-sim` and `mergus-testkit` MAY depend on layers 0 to 4.
   Dev-dependencies are ignored for layers and cycles.
3. MG-REQ-0126.3: a dependency on a crate named `mergus-*` does not resolve to a workspace
   member (MG-REQ-0003).
4. MG-REQ-0126.4: a core crate has a normal or build dependency on a member that is not
   core, other than `mergus-ports` (MG-REQ-0004).
5. MG-REQ-0126.5: a member other than `mergus-runtime`, `mergus-cli`, `mergus-py` and
   `mergus-testkit` declares a normal or build dependency on `mergus-ports` with its feature
   `real` enabled, or `mergus-ports` lists `real` in its default features (MG-REQ-0004,
   MG-REQ-0121, MG-REQ-0133).
6. MG-REQ-0126.6: `mergus-plan` depends, directly or through normal dependencies, on an
   engine, source or adapter crate or on `mergus-runtime` (MG-REQ-0121).
7. MG-REQ-0126.7: a member other than `mergus-security` depends on the `keyring` crate
   (MG-REQ-0123).
8. MG-REQ-0126.8: a member depends on `mergus-sim` or `mergus-testkit` outside
   `[dev-dependencies]` (MG-REQ-0124).

A rule that names a crate that does not exist yet is skipped for that crate.

- Kind: process
- Verify: check `boundaries`
- Decision: 0001
