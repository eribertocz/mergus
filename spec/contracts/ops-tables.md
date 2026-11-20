---
id: contracts-ops-tables
title: Ops tables as a public contract
crate: mergus-ops
phase: "0.7"
status: draft
depends_on: [contracts-overview]
---

# Ops tables

Mergus describes its own work in tables of the `ops` schema, readable with SQL by users, BI
tools and Menura's monitoring. Their columns are defined in `schemas/ops-tables.schema.json`.

| Table | One row per |
|---|---|
| `ops.runs` | Run: status, duration, environment, trigger, published manifest |
| `ops.steps` | Step or segment: rows, bytes, duration, memory, spill, engine |
| `ops.quality` | Test, monitor or integrity check result |
| `ops.profiles` | Column profile per data version |
| `ops.lineage` | Table and column lineage per version |
| `ops.alerts` | Alert: open, resolved, recipients |
| `ops.incidents` | Incident: cause, affected tables, owner, state |
| `ops.loads` | Extraction load: source, mode, rows, watermark, result |

### MG-REQ-0275 · Ops tables follow their schema

Each ops table MUST have exactly the columns and types of its schema version. Adding a column
is a minor change; removing, renaming or retyping one is a major change of the ops contract.

- Kind: behavior
- Verify: test `mg_req_0275_ops_tables_match_schema`

### MG-REQ-0276 · Ops never hold secrets or raw values of PII

Ops tables MUST NOT contain secret values or values of columns labelled `pii` other than their
pseudonymized form.

- Kind: behavior
- Verify: test `mg_req_0276_ops_tables_hold_no_secret_or_pii`
- Depends: MG-REQ-0145

### MG-REQ-0277 · Ops writes never block a publication

A failure to write ops rows MUST raise an incident but MUST NOT change the outcome of the run
or its publication.

- Kind: behavior
- Verify: test `mg_req_0277_ops_failure_does_not_block_publication`
