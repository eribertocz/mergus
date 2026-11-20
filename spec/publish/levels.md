---
id: publish-levels
title: Publication levels by storage capability and safe degradation
crate: mergus-runtime
phase: "0.5"
status: draft
depends_on: [publish-sequence, storage-profiles]
---

# Levels

| Level | Storage | Strategy | MVP |
|---|---|---|---|
| A | Native versions (DuckLake, Iceberg, Delta) | Pinned read by version | DuckLake |
| A-ext | Same, written by another tool (`connect`) | Pin exact version; check it still exists at each publication | yes |
| B | Parquet in immutable folders | Pinned read by file list | after MVP |
| C+ | No versions, multi-table transaction (DuckDB file) | Safe degradation | after MVP |
| C | No versions, tables change separately (ClickHouse) | Atomic swap per table and safe degradation | after MVP |
| F | External without versions (`live`) | Schema only, no evidence | yes |

OLTP databases are never a managed gold store; they can only be sources or `live` tables.

### MG-REQ-0911 · Level per table

Every gold table MUST have a level from this table, computed at stage 2 and stored in the
manifest.

- Kind: behavior
- Verify: test `mg_req_0911_level_stored_per_table`

### MG-REQ-0912 · A-ext existence check

For A-ext tables, every publication MUST verify that the pinned version still exists; if not,
the table's evidence is withdrawn and an incident raised.

- Kind: behavior
- Verify: test `mg_req_0912_external_version_checked`

### MG-REQ-0913 · Safe degradation for C and C+

For levels C and C+, Mergus MUST first publish a reduced state that omits the affected
summaries, the base relations about to change and their checks; then change the data; then
rebuild summaries, run checks and publish the full state.

- Kind: behavior
- Verify: test `mg_req_0913_safe_degradation_sequence`

### MG-REQ-0914 · OLTP never as gold

Configuring an OLTP database as a managed gold store MUST be a validation error.

- Kind: behavior
- Verify: test `mg_req_0914_oltp_gold_rejected`
