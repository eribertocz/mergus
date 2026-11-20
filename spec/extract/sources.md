---
id: extract-sources
title: Declaring sources in sources/<name>.toml
crate: mergus-source
phase: "0.3"
status: draft
depends_on: [project-layout, project-project-toml]
---

# Sources

```toml
# sources/erp.toml
kind = "database"                 # database | files | rest
mode = "copy"                     # copy | connect | live
connection = "secret:erp"
replica = "secret:erp_replica"    # optional
overlap = "1h"

[tables.sales]
key = "sale_id"
incremental = "updated_at"
deletes = "compare"
delete_check = "daily"
delete_window = "90d"
```

| Mode | For | Effect |
|---|---|---|
| `copy` (default) | Raw operational data | Extracted into raw |
| `connect` | Clean, versioned tables (DuckLake, Iceberg, Delta) | Read in place, pinned by version |
| `live` | Clean tables without versions | Queried by the executor at read time |

### MG-REQ-0401 · Source kinds

A source MUST declare `kind` as `database` (through ADBC), `files` (Parquet, CSV, JSON on a
file system or object store) or `rest` (`extract/rest.md`). Other kinds are an error.

- Kind: behavior
- Verify: test `mg_req_0401_source_kind_required`

### MG-REQ-0402 · Tables are declared

Only tables listed under `[tables.<name>]` MUST be extracted. `mergus add source --from <url>`
MAY list the source's tables and write their sections, but Mergus never extracts an
undeclared table.

- Kind: behavior
- Verify: test `mg_req_0402_undeclared_tables_not_extracted`

### MG-REQ-0403 · Raw names

Each declared table MUST land as `raw.<source>__<table>` (current rows) with
`raw.<source>__<table>__changes` (every version and detected delete).

- Kind: behavior
- Verify: test `mg_req_0403_raw_table_names`

### MG-REQ-0404 · Only needed columns and rows travel

Extraction MUST push down to the source the column list used by the project and the
incremental filter, so that unused columns and old rows are not transferred.

- Kind: behavior
- Verify: test `mg_req_0404_projection_and_filter_pushed_down`

### MG-REQ-0405 · Connect and live never copy

Sources in `connect` or `live` mode MUST NOT be copied into raw. `connect` tables are pinned
by version in the manifest; `live` tables are marked live in the manifest and never get
summaries.

- Kind: behavior
- Verify: test `mg_req_0405_connect_and_live_not_copied`

### MG-REQ-0406 · New source columns that look personal wait for a decision

A column that appears in a source after its first extraction MUST pass through the PII
detector (`security/pii.md`). If it looks personal, it MUST NOT be read until it is
classified; extraction of the other columns continues and an incident lists it with the
command to classify it. If it does not look personal, it passes with a notice in the report.

- Kind: behavior
- Verify: test `mg_req_0406_suspected_pii_column_held`
- Phase: 0.6
- Depends: MG-REQ-1032

### MG-REQ-0407 · Breaking source changes stop the table

A dropped column, a renamed column or an incompatible type change in a source table MUST stop
the extraction of that table with a typed error naming the column, the old and the new type.

- Kind: behavior
- Verify: test `mg_req_0407_breaking_source_change_stops_table`
