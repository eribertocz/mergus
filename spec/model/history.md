---
id: model-history
title: History tables (SCD type 2)
crate: mergus-plan
phase: "0.4"
status: draft
depends_on: [model-refresh, storage-raw]
---

# History

```toml
[dim_customer]
refresh = "history"
key = "customer_id"
by = "updated_at"          # or by = ["segment", "city"]
deletes = "close"          # ignore | close | tombstone
```

### MG-REQ-0661 · Required options

A `history` model MUST declare `key` and `by`. `deletes` defaults to `ignore`.

- Kind: behavior
- Verify: test `mg_req_0661_history_requires_key_and_by`

### MG-REQ-0662 · Change detection

With `by = "<column>"` a new version MUST be created when the column's value increases; with
`by = [columns]` when any listed column's value differs from the current version:

1. MG-REQ-0662.1: the detection time is the run's UTC start, to the microsecond, given by the
   caller.
2. MG-REQ-0662.2: "increases" means strictly greater. An incoming NULL never opens a version.
   A non-NULL value over a current NULL opens one.
3. MG-REQ-0662.3: with `by = [columns]`, values are compared with `IS DISTINCT FROM`, so NULL to
   NULL is no change.
4. MG-REQ-0662.4: a key with no current version, because it is new or was closed or
   tombstoned, always opens a version, whatever `by` says. The new version has
   `_is_deleted = false`.

- Kind: behavior
- Verify: test `mg_req_0662_change_detection_modes`

### MG-REQ-0663 · Validity columns

History tables MUST have `_valid_from`, `_valid_to`, `_is_current` and `_is_deleted`. The
current version has `_valid_to = 9999-12-31 00:00:00` so that one range test finds it:

1. MG-REQ-0663.1: the four columns are `TIMESTAMPTZ`, `TIMESTAMPTZ`, `BOOLEAN` and `BOOLEAN`. The
   sentinel is `9999-12-31 00:00:00+00`.
2. MG-REQ-0663.2: a version's `_valid_from` is its `by` value when `by` is one column and that
   value is not NULL. Otherwise it is the row's `_extracted_at` for a row of a changes view,
   and the detection time for any other input.
3. MG-REQ-0663.3: validity is half-open, `_valid_from <= ts < _valid_to`. A replaced version's
   `_valid_to` equals the next version's `_valid_from`, so no instant has two versions of a
   key.
4. MG-REQ-0663.4: each key has at most one version with `_is_current = true`: its latest
   version, which may be a tombstone. Every closed version has `_is_current = false`.
5. MG-REQ-0663.5: a `by` value that is not later than the current version's `_valid_from` does
   not give the new version's `_valid_from`, so no version has an empty or inverted range. The
   detection time does instead.
6. MG-REQ-0663.6: "`by` is one column" means `by = "<column>"`. A list of one column is list
   mode. That column's type must be `DATE`, `TIMESTAMP` or `TIMESTAMPTZ`. Any other type is a
   typed error with the next free `MG-06xx` code, raised before any row is written.
7. MG-REQ-0663.7: a key that appears more than once in a run's input that is not a changes view
   fails the run with a typed error. Nothing is written.

- Kind: behavior
- Verify: test `mg_req_0663_validity_columns`

### MG-REQ-0664 · Deletes

When a key disappears from the input: `ignore` does nothing; `close` sets `_valid_to` of the
current version to the detection time; `tombstone` closes it and adds a version with
`_is_deleted = true`:

1. MG-REQ-0664.1: for a raw input, a key disappears when the changes view has a row for it with
   `_deleted = true`, and the detection time is that row's `_deleted_at`. For any other input,
   a key disappears when it is missing from the whole input, and the detection time is the
   run's.
2. MG-REQ-0664.2: a tombstone has `_valid_from` equal to the detection time, `_valid_to` equal to
   the sentinel, `_is_current = true`, `_is_deleted = true`, the key, and NULL in every other
   column.
3. MG-REQ-0664.3: a key that comes back after `close` or `tombstone` opens a new version, as
   MG-REQ-0662.4 says.
4. MG-REQ-0664.4: refusing a rebuild (MG-REQ-0665) and every history error take the next free
   `MG-06xx` code, with exit code 1.

- Kind: behavior
- Verify: test `mg_req_0664_delete_modes`

### MG-REQ-0665 · Protected from rebuilds

A full rebuild MUST NOT drop or rewrite a history table unless the command carries
`--rebuild-history` and an interactive confirmation (or `--yes`), because history older than
raw retention cannot be recovered.

- Kind: behavior
- Verify: test `mg_req_0665_history_rebuild_needs_flag`

### MG-REQ-0666 · as_of macro

Mergus MUST provide the built-in table macro `as_of(table, ts)` returning the version of each
key valid at `ts`:

1. MG-REQ-0666.1: `as_of` returns the rows with `_valid_from <= ts AND ts < _valid_to` and
   `_is_deleted = false`. A key that is tombstoned at `ts` has no row.

- Kind: behavior
- Verify: test `mg_req_0666_as_of_returns_valid_versions`

### MG-REQ-0667 · History reads the changes view

History models MUST read `raw.<source>__<table>__changes` when their input is a raw table, so
that intermediate versions between runs are kept:

1. MG-REQ-0667.1: a run reads the rows of loads recorded `ok` whose `_load_id` is greater than the
   highest `_load_id` already applied. The caller passes that value from the previous
   successful run's state, and the engine returns the new highest.
2. MG-REQ-0667.2: a key's rows are applied in `_extracted_at` order, then `_load_id` order, each
   one through MG-REQ-0662.
3. MG-REQ-0667.3: the raw metadata columns `_load_id`, `_extracted_at`, `_deleted` and
   `_deleted_at` are not columns of the history table.

- Kind: behavior
- Verify: test `mg_req_0667_history_reads_changes_view`
