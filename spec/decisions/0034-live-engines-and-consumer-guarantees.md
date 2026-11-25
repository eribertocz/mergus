# 0034 · Live only on analytical engines; freshness over mechanism; guarantees to consumers

- Status: accepted
- Date: 2026-11-25
- Answers: the owner's proposal after reviewing the source modes

## Context

The specification allowed `live` on any source, including transactional databases, and
`publish/levels.md` said that OLTP databases could be `live` tables. In `live` mode the
executor queries the source at read time. No rule protected that database: the source-care
rules cover only Mergus's extraction. CDC (phase 1.3) had no principle for how it is declared.
Consumers that cache results had no stated way to know that a table's data had changed.

## Decision

- `connect` and `live` require `engine`, checked against an allowlist (MG-REQ-0409). `live`
  is allowed only on analytical engines: DuckDB, DuckLake, ClickHouse, Iceberg, Delta and
  Parquet. `connect` is allowed only on the versioned formats it pins: DuckLake, Iceberg and
  Delta. Anything else, transactional or unknown, is a validation error that suggests `copy`
  with `fresh`. The unprotected case disappears instead of needing protection.
  `engine` is the user's claim, so each connection checks it against the engine that
  actually answers, and fails typed on a mismatch or an unidentified engine (MG-REQ-0410).
  A secret that points a `live` source at a transactional database is therefore refused. This is the
  direction of mirroring and zero-ETL products, which copy operational data to analytical
  storage rather than querying it live.
- CDC principle: the user declares freshness, not mechanism. A `copy` source can move to CDC
  without changing its models or their `fresh`. The `cdc` value of `deletes` is removed,
  because it declared a mechanism per table; phase 1.3 specifies how CDC is enabled.
- Guarantees to consumers (phase 0.5), and only Mergus's side of them:
  - a data token per table that changes with its data (MG-REQ-0251). It is the version at
    which the table last changed, not the newest catalog snapshot, so writes to other tables
    do not change it;
  - pinned versions readable inside the rollback window, with a typed error after it
    (MG-REQ-0252);
  - detection by reading the pointer (MG-REQ-0247, MG-REQ-0248). A remote event may come
    after the MVP.

## Consequences

The source checks of phase 0.3 gain MG-REQ-0409. The schema of MG-REQ-0376 gains `engine`.
The manifest of phase 0.5 gains `level` and `data_token` per table.
