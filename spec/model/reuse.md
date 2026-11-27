---
id: model-reuse
title: Macros and seeds
crate: mergus-project
phase: "0.4"
status: draft
depends_on: [model-sql]
---

# Reuse: macros and seeds

```sql
-- macros/finance.sql
CREATE MACRO safe_div(a, b) AS CASE WHEN b = 0 THEN NULL ELSE a / b END;
CREATE MACRO clean_sales(t) AS TABLE SELECT * FROM query_table(t) WHERE status <> 'VOID';
```

### MG-REQ-0641 · Macros are DuckDB macros only

Files in `macros/` MUST contain only `CREATE MACRO` statements (scalar or table). Text
templating MUST NOT exist:

1. MG-REQ-0641.1: each macro file is run, through the `Database` port, in a fresh in-memory
   session opened with external access off and the configuration locked. No file, network,
   `ATTACH`, `INSTALL`, `LOAD` or `SET` can then succeed.
2. MG-REQ-0641.2: after the file runs, the session's catalog must hold only new macros:
   `duckdb_tables()`, `duckdb_views()`, `duckdb_sequences()`, `duckdb_types()`,
   `duckdb_indexes()` and `duckdb_schemas()` give the same rows as before the file ran. Any
   statement that DuckDB refuses, and any other object the file creates, is a validation
   error naming the file. Its code is the next free `MG-06xx`.
3. MG-REQ-0641.3: the macros are read from `duckdb_functions()`. They are the rows with
   `internal = false` and `function_type` either `macro` or `table_macro`, each with its
   `macro_definition`.

- Kind: behavior
- Verify: test `mg_req_0641_macros_only_create_macro`

### MG-REQ-0642 · Macro names are unique and safe

Macro names MUST be unique in the project and MUST NOT shadow a DuckDB built-in function:

1. MG-REQ-0642.1: names are compared without case.
2. MG-REQ-0642.2: a built-in is a row of `duckdb_functions()` with `internal = true` in the
   same fresh session before any macro file runs. Extensions that are not loaded are not
   built-ins.
3. MG-REQ-0642.3: each clash is a validation error naming both files, or the file and the
   built-in. Its code is the next free `MG-06xx`.
4. MG-REQ-0642.4: the names of Mergus's own macros, `as_of` (MG-REQ-0666), are reserved like
   built-ins. A project macro with a reserved name is the same validation error.

- Kind: behavior
- Verify: test `mg_req_0642_macro_name_clash_rejected`

### MG-REQ-0643 · Macros loaded per session

Every DuckDB session that runs a segment MUST load the project's macros before executing.

- Kind: behavior
- Verify: test `mg_req_0643_macros_loaded_in_each_session`

### MG-REQ-0644 · Fingerprint by use

Changing a macro MUST change the fingerprint only of models that call it, directly or
through another macro:

1. MG-REQ-0644.1: calls are the function names of the `SqlParser` tree, matched without case
   against the project's macros:
   - for a model, the tree of its query;
   - for a scalar macro, the tree of `SELECT <macro_definition>`;
   - for a table macro, the tree of its `macro_definition`.
2. MG-REQ-0644.2: a model uses every macro reachable from its calls. Each used macro enters
   the fingerprint's `inputs` as `macro:<name>` (the lowercase name), valued
   `sha256:<hex of the SHA-256 of its macro_definition>`.

- Kind: behavior
- Verify: test `mg_req_0644_macro_change_refingerprints_callers_only`

### MG-REQ-0645 · Seeds

Each `seeds/<name>.csv` MUST be declared in `project.toml` as `[seeds.<name>]` with its column
types, and loaded as `raw.seed__<name>`. Its content hash is part of the fingerprint of the
models that read it:

1. MG-REQ-0645.1: `[seeds.<name>]` has the keys `columns`, required, a table of column name to
   DuckDB type name; `delimiter`, a one-character string defaulting to `,`; and `header`, a
   boolean defaulting to `true`. No other key is allowed. Type names are DuckDB column type
   names, as `DESCRIBE` writes them.
2. MG-REQ-0645.2: a CSV with no declaration, a declaration with no CSV, an unknown key and an
   unknown type name are each a validation error. Their codes are the next free `MG-06xx`.
3. MG-REQ-0645.3: a seed's hash is the SHA-256 of its file's bytes. It enters the fingerprint's
   `inputs` of each model that reads `raw.seed__<name>`, valued `sha256:<hex>`.

- Kind: behavior
- Verify: test `mg_req_0645_seed_declared_typed_and_hashed`

### MG-REQ-0646 · Seeds stay small

A seed larger than 10 MB MUST produce a validation warning recommending a source:

1. MG-REQ-0646.1: 10 MB means 10,000,000 bytes. A seed of more bytes warns.
2. MG-REQ-0646.2: a warning is returned with the read project, beside any errors. It has a
   code, a message and the file, and it never fails the read. Its code is the next free
   `MG-06xx`, whose catalog file gives exit code 0.

- Kind: behavior
- Verify: test `mg_req_0646_large_seed_warns`
