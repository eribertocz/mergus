---
id: extract-loads
title: Full and incremental loads, deletes and resuming
crate: mergus-source
phase: "0.3"
status: draft
depends_on: [extract-sources]
---

# Loads

| Table option | Values | Default |
|---|---|---|
| `incremental` | column name | none (full load) |
| `key` | column or list | none |
| `overlap` | duration | source `overlap`, else `1h` |
| `deletes` | `column:<name>` \| `compare` \| `none` \| `cdc` (post-MVP) | none |
| `delete_check` | `every_run` \| `hourly` \| `daily` \| `weekly` | `daily` |
| `delete_window` | duration | whole table |

### MG-REQ-0411 · Full load by default

A table without `incremental` MUST be loaded completely on every extraction.

- Kind: behavior
- Verify: test `mg_req_0411_full_load_without_incremental`

### MG-REQ-0412 · Incremental with overlap

An incremental table MUST read rows whose incremental column is greater than or equal to
`watermark − overlap`, and MUST remove duplicates by `key` so that rows re-read in the
overlap are not duplicated.

- Kind: behavior
- Verify: test `mg_req_0412_overlap_rereads_without_duplicates`

### MG-REQ-0413 · Watermark moves only after commit

The watermark of a table MUST advance only after the extracted rows are committed to raw.

- Kind: behavior
- Verify: test `mg_req_0413_watermark_after_commit`
- Depends: MG-REQ-0143

### MG-REQ-0414 · Incremental needs a key

An incremental table without `key` MUST be a validation error.

- Kind: behavior
- Verify: test `mg_req_0414_incremental_requires_key`

### MG-REQ-0415 · Delete policies

Deletes MUST be detected as declared: `column:<name>` maps the source's logical-delete
column to `_deleted`; `compare` reads only the key columns at the `delete_check` frequency,
within `delete_window` if set, and marks missing keys `_deleted = true`; `none` detects
nothing. Detection time is stored in `_deleted_at`.

- Kind: behavior
- Verify: test `mg_req_0415_delete_policies`

### MG-REQ-0416 · Missing delete policy warns

An incremental table without `deletes` MUST produce a validation warning stating that
deletes in the source will not be seen.

- Kind: behavior
- Verify: test `mg_req_0416_missing_delete_policy_warns`

### MG-REQ-0417 · Keyset chunks

Large reads MUST use keyset pagination (`WHERE key > last ORDER BY key LIMIT n`), never
`OFFSET`, with short transactions.

- Kind: behavior
- Verify: test `mg_req_0417_keyset_pagination`

### MG-REQ-0418 · Resume from the last chunk

An interrupted extraction MUST resume from the last committed chunk, producing exactly the
rows of an uninterrupted extraction (INV-04).

- Kind: behavior
- Verify: test `mg_req_0418_extraction_resumes_from_last_chunk`
- Depends: MG-REQ-0143

### MG-REQ-0419 · Consistent snapshot across tables

Tables of one database source MUST be read from the same point in time when the source
supports shared snapshots (PostgreSQL exported snapshots). Otherwise the time of each
table's read MUST be recorded and the run report MUST show the spread.

- Kind: behavior
- Verify: test `mg_req_0419_tables_share_one_snapshot`

### MG-REQ-0420 · Very late rows are reported

Rows arriving with an incremental value older than `watermark − overlap` MUST be counted and
reported with the suggested `mergus backfill` command.

- Kind: behavior
- Verify: test `mg_req_0420_very_late_rows_reported`
