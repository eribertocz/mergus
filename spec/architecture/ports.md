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
| `SourceConnector` | ADBC connections (MG-REQ-0197) and file readers of sources, returning Arrow |
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
load only the `ducklake` extension, `httpfs` when DuckLake's encrypted mode needs it to write
(MG-REQ-0515), `sqlite` and `postgres` for DuckLake catalogs (MG-REQ-0521.3), and the scanner extensions of MG-REQ-0196 (`postgres` in phase 0.3; `mysql`
and `sqlite` in phase 0.8):

1. MG-REQ-0194.1: at the build that matches the DuckDB of the release's `duckdb` crate and the
   platform, from `extensions.duckdb.org`, with DuckDB's signature check; the version DuckDB
   reports for each loaded extension is recorded in the load registry;
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
- Decision: 0032, 0040, 0041, 0042

### MG-REQ-0195 · ADBC drivers are pinned, verified and cached

- Status: retired
- Reason: database sources are read by DuckDB (MG-REQ-0196); ADBC serves only databases DuckDB
  does not read (MG-REQ-0197). Decision 0040.
- Kind: behavior
- Verify: test `mg_req_0195_adbc_driver_pinned_and_cached`
- Phase: 0.3
- Decision: 0033, 0036, 0038, 0040

### MG-REQ-0196 · Database sources are read by DuckDB

The real `Database` port MUST read every database source that DuckDB can attach with an
extension of the DuckDB team, and every DuckDB file, inside the DuckDB that `mergus-ports`
bundles, with results in Arrow:

1. MG-REQ-0196.1: a DuckDB file is attached with `ATTACH '<location>' (READ_ONLY)`, with no
   extension;
2. MG-REQ-0196.2: PostgreSQL is attached with the `postgres` extension of MG-REQ-0194 as
   `ATTACH '' AS <alias> (TYPE postgres, SECRET <secret>, READ_ONLY)`. The connection value of
   MG-REQ-1001 goes into a temporary DuckDB secret of `TYPE postgres`, never into the attach
   string, never a persistent secret, and the secret is dropped when the source is detached;
3. MG-REQ-0196.3: for each attached PostgreSQL source, `pg_pool_max_connections` is set to the
   source's `max_connections` and `pg_pool_acquire_mode` to `'wait'` before the attach, so
   that the extension never opens more connections than MG-REQ-0427 allows, and
   `pg_statement_timeout_millis` is set from `query_timeout` (MG-REQ-0429). DuckDB documents
   that the pool "can be temporary exceeded when parallel scans are used", so the end-to-end
   test of MG-REQ-0427 counts the connections on a real PostgreSQL server during a parallel
   scan; if the count ever exceeds `max_connections`, parallel `ctid` scans are turned off
   (`pg_use_ctid_scan = false`) for every source;
4. MG-REQ-0196.4: MySQL (`mysql` extension) and SQLite (`sqlite` extension) follow the same
   pattern in phase 0.8, once their read-only session and timeouts are specified.

Its simulated implementation answers from results recorded with the real one and injects the
faults of `architecture/testing.md`.

- Kind: behavior
- Verify: test `mg_req_0196_database_sources_read_by_duckdb`
- Phase: 0.3
- Depends: MG-REQ-0194, MG-REQ-1001
- Decision: 0040

### MG-REQ-0197 · ADBC drivers for databases DuckDB does not read

A database that no DuckDB team extension reads, such as Snowflake, BigQuery or SQL Server,
MUST be read through an ADBC driver loaded by the real `SourceConnector`, which alone depends
on the ADBC crates (MG-REQ-0186). Each driver is pinned by URL and SHA-256 per operating system
and architecture in the Mergus release, refused when its sum differs, cached in
`mergus/adbc/drivers/` under the user cache directory and loaded from there with no network;
a driver that is missing and cannot be fetched fails with a typed error that names the
database, the host and the cache directory.

- Kind: behavior
- Verify: test `mg_req_0197_adbc_driver_pinned_and_cached`
- Phase: 0.8
- Depends: MG-REQ-0186
- Decision: 0033, 0040
