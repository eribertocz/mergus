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
  ids.toml                optional identifiers of renamed models and sources (project/identifiers.md)
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

For Python models, MG-REQ-0307 is met by MG-REQ-0614; for macro calls, by MG-REQ-0642. Both
arrive in phase 0.4. The two rules below apply to `.sql` model files, on the tree of
MG-REQ-0193.

### MG-REQ-0373 · What a SQL model references

Every base-table reference of the tree that does not name a CTE visible at that point
(MG-REQ-0602) MUST resolve, compared without case, as follows; any other reference MUST be
the error of MG-REQ-0307 with the file, line and column of the reference.

1. MG-REQ-0373.1: without schema: a model of the project;
2. MG-REQ-0373.2: in schema `raw`: `<source>__<table>` for a `[tables.<table>]` of
   `sources/<source>.toml`, or `seed__<name>` for a `[seeds.<name>]` of `project.toml`.

A reference with a catalog, or with a schema other than `raw`, never resolves.

- Kind: behavior
- Verify: test `mg_req_0373_references_resolved`
- Depends: MG-REQ-0307, MG-REQ-0193
- Decision: 0023

### MG-REQ-0374 · What read-only SQL rejects

For MG-REQ-0308, a `.sql` model MUST be an error naming the file, line, column and the
rejected construct, with exit code 1, when:

1. MG-REQ-0374.1: the parser reports an error, or the text holds other than exactly one
   statement (`json_serialize_sql` accepts only `SELECT`, so `ATTACH`, `COPY`, `INSTALL`,
   `LOAD`, `SET`, `PRAGMA` and every write fail here);
2. MG-REQ-0374.2: it calls a table function other than `range`, `generate_series` and
   `unnest` (this rejects `read_csv`, `read_parquet`, `glob`, `query` and the like);
3. MG-REQ-0374.3: a base-table name is not a name of MG-REQ-0305 (this rejects `FROM
   'file.csv'` and URLs);
4. MG-REQ-0374.4: it calls `getenv`, `nextval`, `currval` or `setseed`.

- Kind: behavior
- Verify: test `mg_req_0374_side_effects_rejected`
- Depends: MG-REQ-0308, MG-REQ-0193
- Decision: 0023


### MG-REQ-0375 · Models form no cycle

The references of MG-REQ-0373 MUST NOT form a cycle between models. A cycle MUST be an error
with exit code 1 naming every model of the cycle in reference order, starting from the first in
name order, with the file and line of each reference that closes it:

1. MG-REQ-0375.1: `.sql` and `.py` models are checked together, in one pass when the project
   is read, so a cycle through a `.py` model (`a.sql` → `b.py` → `a.sql`) is found there too.
   A `.py` model's references are the parameters of its steps (MG-REQ-0614).
2. MG-REQ-0375.2: the line of a `.sql` reference is the line of the referenced name in the
   parsed SQL, never a word in a comment or a string literal. The line of a `.py` reference is
   the line of the parameter that names the model.

- Kind: behavior
- Verify: test `mg_req_0375_model_cycle_rejected`
- Phase: 0.2
- Depends: MG-REQ-0373

### MG-REQ-0359 · Closest known name

When an error names the closest known name (MG-REQ-0307, MG-REQ-0316, MG-REQ-0331), Mergus MUST
choose, among the candidates of that error, the one with the smallest Levenshtein distance,
counted in Unicode scalar values, to the name given; ties are broken by byte order. It MUST
suggest it only when that distance is at most `max(2, n / 3)`, `n` being the number of scalar
values of the name given and the division rounding down. Otherwise the error has no suggestion.

- Kind: behavior
- Verify: test `mg_req_0359_closest_name_by_edit_distance`
- Decision: 0017

### MG-REQ-0360 · Suggested valid name

The suggestion of MG-REQ-0305 MUST be the name given after these steps, in order: ASCII
letters lowercased; every run of characters outside `a-z`, `0-9` and `_` replaced by one `_`;
leading and trailing `_` removed; `n_` prepended when the result is empty or starts with a
digit; the result cut to 63 characters.

- Kind: behavior
- Verify: test `mg_req_0360_invalid_name_gets_valid_suggestion`
- Decision: 0017
