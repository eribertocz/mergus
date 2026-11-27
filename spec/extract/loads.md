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
| `deletes` | `column:<name>` \| `compare` \| `none` | none |
| `delete_check` | `every_run` \| `hourly` \| `daily` \| `weekly` | `daily` |
| `delete_window` | duration | whole table |

CDC sources come in phase 1.3 (`roadmap.md`). Their principle is fixed now (decision 0034):
the user declares freshness (`fresh`), not the capture mechanism, so a declared `copy` source
can move to CDC without changing its models or their `fresh`.

### MG-REQ-0411 · Full load by default

A table without `incremental` MUST be loaded completely on every extraction.

- Kind: behavior
- Verify: test `mg_req_0411_full_load_without_incremental`

### MG-REQ-0412 · Incremental with overlap

An incremental table MUST read rows whose incremental column is greater than or equal to
`watermark − overlap`, and MUST remove duplicates by `key` so that rows re-read in the
overlap are not duplicated.

1. MG-REQ-0412.1: the watermark is the largest non-NULL incremental value among a load's
   committed rows; rows with a NULL incremental value are loaded and do not move it, and a
   load with no non-NULL value keeps the previous watermark; after the first load, rows whose
   incremental value is NULL are read again only by a full load;
2. MG-REQ-0412.2: `overlap` is subtracted only from `DATE`, `TIMESTAMP` (read as UTC) and
   `TIMESTAMPTZ` columns, from a `DATE` column as whole days, the overlap rounded up to the
   next day; any other incremental column is read from `>= watermark`;
3. MG-REQ-0412.3: a re-read row whose source columns equal the current row of its key is not
   appended to `__changes` again; a row that changed is a new version.

- Kind: behavior
- Verify: test `mg_req_0412_overlap_rereads_without_duplicates`
- Decision: 0047, 0049

### MG-REQ-0413 · Watermark moves only after commit

The watermark of a table MUST advance only after the extracted rows are committed to raw.
It is stored as `watermark_after` of `ops.loads` (MG-REQ-0514), in the text DuckDB gives for
`CAST(value AS VARCHAR)`, and is cast back to the column's type to read.

- Kind: behavior
- Verify: test `mg_req_0413_watermark_after_commit`
- Decision: 0047, 0049
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

1. MG-REQ-0415.1: with no `deletes`, nothing is detected, as with `none`;
2. MG-REQ-0415.2: `column:<name>` treats `true`, or any non-NULL value of a non-boolean column,
   as deleted; the column stays in raw; a deleted row leaves the current table and is
   appended to `__changes` as MG-REQ-0518.2 says; a source without the column fails with a
   typed error (`MG-04xx`);
3. MG-REQ-0415.3: `compare` is due when `delete_check` is `every_run`, or when 1 hour, 24 hours
   or 7 days have passed since the last compare of the table, the latest non-NULL
   `deletes_checked_at` of its `ops.loads` rows; `delete_window` filters the incremental column
   on both sides; `_deleted_at` is the load's `_extracted_at` (MG-REQ-0518.2);
4. MG-REQ-0415.4: `mergus validate` refuses, with a typed error (`MG-04xx`), `compare` on a
   table without `key`, and `delete_window` on a table without `incremental`; a `delete_window`
   whose `incremental` column is not a date or time type is refused with the same error when
   the table is first read, since only the source knows the type; a delete detected for a key
   that is not current is ignored.

- Kind: behavior
- Verify: test `mg_req_0415_delete_policies`
- Decision: 0047, 0049

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

1. MG-REQ-0418.1: each chunk of a load is a commit that appends to `__changes` with the load's
   `load_id` and `_extracted_at`; the load's last commit rewrites the current table from its
   rows (MG-REQ-0518.1), and only then is its `ops.loads` row written;
2. MG-REQ-0418.2: a `load_id` found in `__changes` with no `ops.loads` row is unfinished; the
   next extraction of that table resumes it with the same `load_id`, after the largest key
   it holds; it is not recorded as failed; when its last commit was already made, the next
   extraction only writes its `ops.loads` row;
3. MG-REQ-0418.3: a table without `key` cannot resume by chunk: its unfinished rows are
   ignored by every reader and it is read again whole.

- Kind: behavior
- Verify: test `mg_req_0418_extraction_resumes_from_last_chunk`
- Decision: 0047, 0049
- Depends: MG-REQ-0143

### MG-REQ-0419 · Consistent snapshot across tables

Tables of one database source MUST be read from the same point in time when the source
supports shared snapshots (PostgreSQL exported snapshots). Otherwise the time of each
table's read MUST be recorded and the run report MUST show the spread.

1. MG-REQ-0419.1: the read of a table runs from the start of its first statement to the end
   of its last one; both times are recorded, in UTC;
2. MG-REQ-0419.2: the spread of a source is the latest end minus the earliest start of its
   tables' reads;
3. MG-REQ-0419.3: through the DuckDB `postgres` extension of phase 0.3, no official page
   documents a snapshot shared by several attached tables, so PostgreSQL sources record times
   and spread.

- Kind: behavior
- Verify: test `mg_req_0419_tables_share_one_snapshot`
- Decision: 0045

### MG-REQ-0420 · Very late rows are reported

Rows arriving with an incremental value older than `watermark − overlap` MUST be counted and
reported with the suggested `mergus backfill` command.

1. MG-REQ-0420.1: very late rows are found by a `compare` of the table: source keys absent
   from raw whose incremental value is older than `watermark − overlap`; they are counted, not
   loaded;
2. MG-REQ-0420.2: a count above zero is a warning with its own `MG-04xx` code that never
   changes the exit code: `table <source>.<table>: <n> rows older than <since> arrived late;
   run mergus backfill --source <source> --table <table> --from <oldest>`; the command is
   specified with backfill of raw in a later phase.

- Kind: behavior
- Verify: test `mg_req_0420_very_late_rows_reported`
- Decision: 0047, 0049
