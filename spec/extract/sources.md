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
| `live` | Clean tables without versions, on an analytical engine only (MG-REQ-0409) | Queried by the executor at read time |

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

### MG-REQ-0409 · Connect and live only on analytical engines

A source in `connect` or `live` mode, and every source of `kind = "database"`, MUST declare
`engine`, checked against an allowlist:

1. MG-REQ-0409.1: `live`: `duckdb`, `ducklake`, `clickhouse`, `iceberg`, `delta` or
   `parquet`;
2. MG-REQ-0409.2: `connect`: `ducklake`, `iceberg` or `delta`, the formats with native
   versions that `connect` pins;
3. MG-REQ-0409.3: any other engine for `connect` or `live`, transactional databases and
   unknown engines included, MUST be a validation error whose fix suggests `mode = "copy"`
   with `fresh` on the models that need current data;
4. MG-REQ-0409.4: in a `copy` source of `kind = "database"`, `engine` names the ADBC driver
   and MUST be one with a pinned driver (MG-REQ-0195): `postgresql` or `duckdb`. A `copy`
   source of another kind MUST NOT set `engine`.

A query of the executor therefore never reaches a transactional database: those are read
only by extraction, under `extract/source-care.md`.

- Kind: behavior
- Verify: test `mg_req_0409_live_and_connect_only_on_analytical_engines`
- Depends: MG-REQ-0376, MG-REQ-0405
- Decision: 0034, 0036

### MG-REQ-0410 · The connected engine matches the declared one

Each time Mergus connects to a `connect` or `live` source, to read a table's schema or pin its
version, it MUST identify the engine that answers, from the driver, the protocol or the
table's metadata, before reading any data. If it differs from the declared `engine`, the
source MUST fail with a typed error that names the source, the declared engine and the
detected one, with exit code 1 and no data read. An engine that cannot be identified counts
as different. For `engine = "duckdb"`, the engine is identified by the ADBC driver's
`ADBC_INFO_VENDOR_NAME`, which MUST be `duckdb`, and `ADBC_INFO_VENDOR_VERSION` is recorded
in the load registry.

- Kind: behavior
- Verify: test `mg_req_0410_connected_engine_matches_declared`
- Depends: MG-REQ-0409
- Decision: 0034

### MG-REQ-0421 · DuckDB files as sources

A source with `kind = "database"` and `engine = "duckdb"` MUST read the `.duckdb` file at
`location` through the DuckDB ADBC driver of MG-REQ-0195.4, opened read-only
(`access_mode = READ_ONLY`), so Mergus never writes to it. With `mode = "copy"` its declared
tables are extracted into raw like any database source. With `mode = "live"` the executor
reads the file at query time (MG-REQ-0405, level F). Each file is one source: data kept in
one file per year is declared as one source per file.

- Kind: behavior
- Verify: test `mg_req_0421_duckdb_file_read_only_through_adbc`
- Phase: 0.3
- Depends: MG-REQ-0195, MG-REQ-0409
- Decision: 0036
