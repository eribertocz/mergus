# 0040 · DuckDB reads the database sources; ADBC only where DuckDB does not

- Status: accepted
- Date: 2026-11-25
- Answers: #184; the owner's rule "when both are official, prefer the native, fast and
  modern one"

## Context

Decisions 0033, 0036 and 0038 read every database source through ADBC: the PostgreSQL driver
fetched as a Python wheel, and the DuckDB driver loaded from the bundled DuckDB through
`duckdb_adbc_init`. The DuckDB case cannot be built without `unsafe` code in `mergus-ports`
(#184). The owner asked which databases each project supports officially, and to prefer the
native, fast and modern option where both do.

Verified facts, from the official pages of DuckDB 2.0 and Apache Arrow ADBC:

- ADBC "focuses primarily on the ADBC standard and client libraries, not on developing drivers
  and shipping driver binaries". Apache maintains stable drivers for PostgreSQL, SQLite and
  Flight SQL; MySQL, SQL Server and Snowflake drivers come from the ADBC Driver Foundry
  (https://arrow.apache.org/adbc/current/driver/index.html). Its PostgreSQL driver
  "executes queries with `COPY` for best performance"
  (https://arrow.apache.org/adbc/current/driver/postgresql.html).
- The DuckDB team maintains the `postgres`, `mysql` and `sqlite` extensions, tier Secondary:
  "supported on a best-effort basis … frequent bugfixes/updates and are shipped with new
  DuckDB releases" (https://duckdb.org/docs/2.0/core_extensions/overview.html).
- The `postgres` extension reads with binary `COPY` (`pg_use_binary_copy`), scans in parallel
  by `ctid` (`pg_use_ctid_scan`), pushes filters and queries down to the server, attaches
  `READ_ONLY`, takes credentials from a `TYPE postgres` secret, limits its pool with
  `pg_pool_max_connections` and `pg_pool_acquire_mode`, and sets `pg_statement_timeout_millis`
  on its scan connections (https://duckdb.org/docs/2.0/core_extensions/postgres/overview.html,
  `.../postgres/connection_pool.html`). The pool "can be temporary exceeded when parallel
  scans are used".
- A DuckDB file attaches read-only with `ATTACH 'file.db' (READ_ONLY)`
  (https://duckdb.org/docs/2.0/sql/statements/attach.html).
- Extensions come from `extensions.duckdb.org`, are signed and verified with DuckDB's built-in
  keys, and each build is "tied both to a specific DuckDB version and a platform"
  (https://duckdb.org/docs/2.0/extensions/extension_distribution.html).
- Neither site publishes benchmarks.

## Decision

- Every database source that a DuckDB team extension reads, and every DuckDB file, is read by
  the DuckDB that Mergus bundles, through the `Database` port (MG-REQ-0196). Phase 0.3 covers
  DuckDB files and PostgreSQL. MySQL and SQLite follow in phase 0.8, once their read-only
  session and timeouts are specified: DuckDB documents no remote query timeout for MySQL,
  and no `READ_ONLY` attach on its SQLite page.
- The extensions are installed and cached like `ducklake` (MG-REQ-0194): the build of the
  bundled DuckDB, signature checked, never autoloaded.
- ADBC stays for databases no DuckDB team extension reads (MG-REQ-0197), in phase 0.8.
- MG-REQ-0195 is retired. Decision 0038 is superseded; decision 0033 now applies only to
  MG-REQ-0197.

## Consequences

- No `unsafe` code, no per-platform driver downloads and no wheel unpacking in phase 0.3.
- One engine reads every source, with Arrow results, parallel scans and pushdown.
- The extensions are best-effort tier; a pinned DuckDB release fixes their build, and the
  end-to-end suite runs against a real PostgreSQL.
- The `SourceConnector` port keeps file readers and, from phase 0.8, ADBC.
