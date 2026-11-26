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

Every raw table MUST have `_load_id`, `_extracted_at`, `_deleted` and `_deleted_at`:

1. MG-REQ-0512.1: `_load_id` is `VARCHAR`, the load's id in the load registry (MG-REQ-0514);
2. MG-REQ-0512.2: `_extracted_at` is `TIMESTAMPTZ`, in UTC, the time the load's extraction
   started;
3. MG-REQ-0512.3: `_deleted` is a `BOOLEAN` that is never NULL, and `_deleted_at` a
   `TIMESTAMPTZ`, in UTC, NULL unless `_deleted` is true.

- Kind: behavior
- Verify: test `mg_req_0512_raw_metadata_columns`
- Decision: 0044

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

Raw MUST be stored encrypted at rest using the storage's encryption mode, DuckLake encrypted
mode for the MVP:

1. MG-REQ-0515.1: the raw DuckLake catalog is created with the `ENCRYPTED` attach option, so
   every Parquet data and delete file is encrypted with a key DuckLake generates per file and
   keeps in the catalog;
2. MG-REQ-0515.2: because the catalog holds the keys, it is protected as a secret: its
   credentials are those of the raw zone (MG-REQ-0501), resolved by `mergus-security`, and a
   local catalog file is never placed inside the project folder: a `local` or `shared` raw
   catalog whose path resolves inside the project root is refused before anything is attached,
   with a typed error (`MG-05xx`) naming the path and suggesting a directory outside the
   project;
3. MG-REQ-0515.3: attaching an existing raw catalog that is not encrypted fails with a typed
   error (`MG-05xx`) naming the catalog; Mergus never writes raw unencrypted.

- Kind: behavior
- Verify: test `mg_req_0515_raw_encrypted_at_rest`
- Decision: 0041, 0044
- Decision: 0041

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

1. MG-REQ-0518.1: both are tables of the raw catalog, written through the storage adapter in
   the same commit: each load appends its rows to `__changes` and rewrites the current table
   (`replace`), so no reader sees one without the other;
2. MG-REQ-0518.2: a detected delete is a row of `__changes` with the last known values of the
   deleted row, `_deleted` true, and the `_load_id` and `_extracted_at` of the load that
   detected it, with `_deleted_at` equal to that `_extracted_at`;
3. MG-REQ-0518.3: a load that returns the same key twice fails with a typed error
   (`MG-05xx`) naming the source, the table and the key, and writes nothing.

- Kind: behavior
- Verify: test `mg_req_0518_current_and_changes_views`
- Decision: 0044
- Depends: MG-REQ-0403
