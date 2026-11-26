# 0033 · ADBC drivers pinned, verified and cached

- Status: accepted; narrowed by 0040 to databases DuckDB does not read
- Date: 2026-11-24
- Answers: needs-owner #160 (the owner chose option A)

## Context

MG-REQ-0401 reads database sources through ADBC, which covers many databases (PostgreSQL,
SQL Server, Snowflake, BigQuery and more). ADBC drivers are native libraries, one per
operating system and architecture, and they are not on crates.io.

## Decision

- ADBC stays, as the specification already said. The real `SourceConnector` loads pinned
  driver libraries through the ADBC driver manager. Only `mergus-ports` depends on the ADBC
  crates (MG-REQ-0126.7).
- Each driver is fetched from the Apache Arrow ADBC release artifacts. Its URL and SHA-256 are
  pinned per platform in the release. It is cached under the user cache directory and works
  offline after the first use, like the DuckLake extension (MG-REQ-0194).
- PostgreSQL is the first driver, in phase 0.3. Other databases add a pinned driver each.

## Consequences

CI needs the host of the ADBC release artifacts. Tests that need a real database run with
the end-to-end suite (`architecture/testing.md`); pull request tests use the simulated
`SourceConnector`.
