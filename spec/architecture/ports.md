---
id: architecture-ports
title: Ports that isolate the core from the outside world
crate: mergus-ports
phase: "0.1"
status: draft
depends_on: [architecture-boundaries]
---

# Ports

Everything that touches the outside world goes through a trait of `mergus-ports`. Production
wires the real implementation; tests and the simulator wire simulated ones.

| Port | Covers |
|---|---|
| `Clock` | Current time, monotonic time, sleeping |
| `FileSystem` | Local files and directories |
| `ObjectStore` | S3-compatible and local object storage |
| `Network` | HTTP and TCP connections |
| `SourceConnector` | ADBC connections and file readers of sources |
| `Random` | Random numbers and identifiers |
| `Process` | Child processes |
| `Environment` | Environment variables and the user configuration directory |
| `SqlParser` | DuckDB's SQL parser, without executing anything |

### MG-REQ-0130 · Only ports touch the world

Outside `mergus-ports`, code MUST NOT call `std::time::SystemTime`, `std::time::Instant`,
`std::fs`, `std::env`, `std::net`, `std::process` or a random-number generator directly.
Clippy's `disallowed_methods` and `disallowed_types` enforce it.

- Kind: process
- Verify: check `clippy`

### MG-REQ-0131 · Every port has a simulated implementation

Each port MUST have a simulated implementation in `mergus-sim` that is deterministic for a
given seed and can inject the faults listed in `architecture/testing.md`.

- Kind: behavior
- Verify: test `mg_req_0131_every_port_is_simulated`

### MG-REQ-0132 · Ports are passed, not global

Ports MUST be passed explicitly (constructor or context argument). Global or thread-local
port instances MUST NOT exist.

- Kind: process
- Verify: review `explicit-ports`

### MG-REQ-0133 · Real implementations behind a feature

`mergus-ports` MUST compile its traits without any feature, and its real implementations
only under the non-default Cargo feature `real`. Only the crates allowed by MG-REQ-0126.5
enable it.

- Kind: process
- Verify: check `boundaries`
- Decision: 0001
- Depends: MG-REQ-0126

### MG-REQ-0193 · SQL is parsed by DuckDB through a port

Crates MUST NOT parse SQL with their own grammar. The `SqlParser` port MUST return, for one
SQL text, the JSON tree that DuckDB's `json_serialize_sql` produces for it. Its real
implementation MUST use an in-memory DuckDB of the release's `duckdb` crate version with
`enable_external_access = false`, and MUST NOT execute the text. Its simulated implementation
returns trees recorded from the real one.

- Kind: behavior
- Verify: test `mg_req_0193_sql_parsed_without_execution`
- Phase: 0.2
- Decision: 0023
