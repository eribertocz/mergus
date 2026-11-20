---
id: project-layout
title: Project directory layout and what the location of a file means
crate: mergus-project
phase: "0.2"
status: draft
depends_on: [architecture-overview]
---

# Project layout

```
my_project/
  project.toml            the project: gold location, environments, seeds, security, version pin
  sources/<source>.toml   one file per source (database, files or REST API)
  models/                 one model per file; any depth of folders
    <folder>/mergus.toml  optional configuration of the models of that folder
    gold/<schema>/<table>.sql|.py    published models
  macros/*.sql            DuckDB CREATE MACRO statements only
  lib/*.py                reusable Python functions
  seeds/*.csv             small fixed reference tables
  contracts/<model>.json  locked output contracts (generated)
  ossie/                  semantic model files, opaque to Mergus
```

### MG-REQ-0301 · Code is only code

A `.sql` model file MUST contain only DuckDB SQL and a `.py` model file only Python. No
comment, decorator, import from Mergus or call has configuration meaning. Configuration
lives only in TOML files.

- Kind: behavior
- Verify: test `mg_req_0301_comments_have_no_meaning`

### MG-REQ-0302 · One model, one file, one table

Each file under `models/` with extension `.sql` or `.py` MUST define exactly one model, named
after the file stem, producing exactly one table.

- Kind: behavior
- Verify: test `mg_req_0302_one_file_defines_one_model`

### MG-REQ-0303 · Model names are unique

Model names MUST be unique in the project regardless of folder. A duplicate MUST be reported
with both paths (exit code 1).

- Kind: behavior
- Verify: test `mg_req_0303_duplicate_model_names_rejected`

### MG-REQ-0304 · The folder decides publication

A model at `models/gold/<schema>/<table>.(sql|py)` MUST be published as
`gold.<schema>.<table>` unless its configuration says otherwise (`publish`). Every other
model is intermediate.

- Kind: behavior
- Verify: test `mg_req_0304_gold_path_defines_published_name`

### MG-REQ-0305 · Names are lowercase identifiers

Model, folder, schema, source and seed names MUST match `^[a-z][a-z0-9_]{0,62}$`. Others are
an error with the closest valid suggestion.

- Kind: behavior
- Verify: test `mg_req_0305_invalid_names_rejected_with_suggestion`

### MG-REQ-0306 · One source, one file

Each source MUST be declared in its own file `sources/<source>.toml`; the source name is the
file stem. Sources MUST NOT be declared in `project.toml`.

- Kind: behavior
- Verify: test `mg_req_0306_source_name_is_file_stem`

### MG-REQ-0307 · Code can name only what the project knows

SQL and Python models MUST reference only models, raw tables of declared sources, seeds and
macros of the project. Any other name MUST be an error naming the closest known name.

- Kind: behavior
- Verify: test `mg_req_0307_unknown_reference_rejected`

### MG-REQ-0308 · User SQL is read-only

User SQL MUST NOT write, `ATTACH`, load extensions, change settings or access the network or
the file system. Such statements are rejected at validation.

- Kind: behavior
- Verify: test `mg_req_0308_side_effect_sql_rejected`
