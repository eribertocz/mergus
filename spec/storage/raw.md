---
id: storage-raw
title: The raw layer
crate: mergus-runtime
phase: "0.3"
status: draft
depends_on: [storage-zones, extract-sources]
---

# Raw

Extraction produces Arrow batches; the storage adapter writes them (MG-REQ-0122).

### MG-REQ-0511 · Native types, never text

Raw tables MUST keep the source's columns with their mapped native types. Rows MUST NOT be
stored as JSON or text blobs, except REST payload fields the user maps that way.

- Kind: behavior
- Verify: test `mg_req_0511_raw_keeps_native_types`

### MG-REQ-0512 · Metadata columns

Every raw table MUST have `_load_id`, `_extracted_at`, `_deleted` and `_deleted_at`.

- Kind: behavior
- Verify: test `mg_req_0512_raw_metadata_columns`

### MG-REQ-0513 · Each load is immutable

Each load MUST be an immutable version of the raw table. Versions older than the retention
are expired by maintenance, never by a run.

- Kind: behavior
- Verify: test `mg_req_0513_loads_are_immutable_versions`

### MG-REQ-0514 · Load registry

Every load MUST be recorded in `ops.loads` with `load_id`, source, table, mode, rows,
watermark before and after, duration and result.

- Kind: behavior
- Verify: test `mg_req_0514_every_load_registered`

### MG-REQ-0515 · Raw is encrypted

Raw MUST be stored encrypted at rest using the storage's encryption mode (DuckLake encrypted
mode for the MVP), with keys from `security/`.

- Kind: behavior
- Verify: test `mg_req_0515_raw_encrypted_at_rest`

### MG-REQ-0516 · Raw retention

Raw retention MUST default to 30 days of loads and be configurable per source. History tables
rebuilt from raw can only recover changes inside this retention.

- Kind: behavior
- Verify: test `mg_req_0516_raw_retention_default_and_override`

### MG-REQ-0517 · Raw can be turned off per source

A source MAY set `raw = false`; its models then read the source in the same run and cannot
be reprocessed without the source. The choice MUST be recorded and shown by `plan`.

- Kind: behavior
- Verify: test `mg_req_0517_raw_off_is_recorded`

### MG-REQ-0518 · Current and changes views

`raw.<source>__<table>` MUST show only current, not deleted rows per key (for tables without
`key`, the rows of the latest load);
`raw.<source>__<table>__changes` MUST show every version and detected delete.

- Kind: behavior
- Verify: test `mg_req_0518_current_and_changes_views`
- Depends: MG-REQ-0403
