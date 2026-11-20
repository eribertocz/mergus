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
`by = [columns]` when any listed column's value differs from the current version.

- Kind: behavior
- Verify: test `mg_req_0662_change_detection_modes`

### MG-REQ-0663 · Validity columns

History tables MUST have `_valid_from`, `_valid_to`, `_is_current` and `_is_deleted`. The
current version has `_valid_to = 9999-12-31 00:00:00` so that `BETWEEN` works.

- Kind: behavior
- Verify: test `mg_req_0663_validity_columns`

### MG-REQ-0664 · Deletes

When a key disappears from the input: `ignore` does nothing; `close` sets `_valid_to` of the
current version to the detection time; `tombstone` closes it and adds a version with
`_is_deleted = true`.

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
key valid at `ts`.

- Kind: behavior
- Verify: test `mg_req_0666_as_of_returns_valid_versions`

### MG-REQ-0667 · History reads the changes view

History models MUST read `raw.<source>__<table>__changes` when their input is a raw table, so
that intermediate versions between runs are kept.

- Kind: behavior
- Verify: test `mg_req_0667_history_reads_changes_view`
