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
| `SourceConnector` | ADBC connections and file readers of sources, returning Arrow |
| `Random` | Random numbers and identifiers |
| `Process` | Child processes |
| `Environment` | Command-line arguments, current directory, environment variables and the user configuration directory |
| `SqlParser` | DuckDB's SQL parser, without executing anything |
| `Database` | In-process DuckDB connections: run SQL, read and write Arrow, attach DuckLake catalogs |
| `Keychain` | Entries of the operating system keychain: read, write and delete by service and account |

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

### MG-REQ-0194 · DuckDB only through the Database port

Crates other than `mergus-ports` MUST NOT depend on the `duckdb` crate; they reach DuckDB
through the `Database` port (MG-REQ-0186). Its real implementation MUST disable the automatic
installing and loading of extensions, MUST NOT allow unsigned extensions, and MAY install and
load only the `ducklake` extension:

1. MG-REQ-0194.1: at the version that matches the DuckDB of the release's `duckdb` crate, from
   `extensions.duckdb.org`, with DuckDB's signature check;
2. MG-REQ-0194.2: into `mergus/duckdb/extensions/` under the user cache directory, the first
   time it is needed, and from there with no network on later uses;
3. MG-REQ-0194.3: when it is missing and cannot be downloaded, with a typed error that names
   the host and the cache directory.

Its simulated implementation answers from results recorded with the real one, as the
simulated `SqlParser` does, and injects the faults of `architecture/testing.md`.

- Kind: behavior
- Verify: test `mg_req_0194_ducklake_extension_pinned_and_cached`
- Phase: 0.3
- Depends: MG-REQ-0186
- Decision: 0032

### MG-REQ-0195 · ADBC drivers are pinned, verified and cached

Database sources MUST be read through ADBC by the real `SourceConnector`, which alone
depends on the ADBC crates (MG-REQ-0186). It loads only the drivers that the Mergus release
pins, starting with PostgreSQL in phase 0.3:

1. MG-REQ-0195.1: each driver library is fetched from the release artifacts of Apache Arrow
   ADBC, at a URL and a SHA-256 sum pinned per operating system and architecture in the
   Mergus release, and is refused when its sum differs;
2. MG-REQ-0195.2: it is stored in `mergus/adbc/drivers/` under the user cache directory the
   first time a source needs it, and loaded from there with no network on later uses;
3. MG-REQ-0195.3: a driver that is missing and cannot be fetched, or a database with no
   pinned driver, fails with a typed error that names the database, the host and the cache
   directory.

4. MG-REQ-0195.4: the DuckDB driver is not fetched: it is the ADBC driver compiled into the
   DuckDB that `mergus-ports` already bundles (MG-REQ-0194), loaded statically through its
   entry point `duckdb_adbc_init`. Its version is therefore the bundled DuckDB's, fixed by
   the `duckdb` crate pin, and the driver MUST report `ADBC_INFO_VENDOR_VERSION` equal to it.

Its simulated implementation serves Arrow batches from in-memory tables and injects the
faults of `architecture/testing.md`.

- Kind: behavior
- Verify: test `mg_req_0195_adbc_driver_pinned_and_cached`
- Phase: 0.3
- Depends: MG-REQ-0186, MG-REQ-0401
- Decision: 0033, 0036

