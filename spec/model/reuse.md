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
templating MUST NOT exist.

- Kind: behavior
- Verify: test `mg_req_0641_macros_only_create_macro`

### MG-REQ-0642 · Macro names are unique and safe

Macro names MUST be unique in the project and MUST NOT shadow a DuckDB built-in function.

- Kind: behavior
- Verify: test `mg_req_0642_macro_name_clash_rejected`

### MG-REQ-0643 · Macros loaded per session

Every DuckDB session that runs a segment MUST load the project's macros before executing.

- Kind: behavior
- Verify: test `mg_req_0643_macros_loaded_in_each_session`

### MG-REQ-0644 · Fingerprint by use

Changing a macro MUST change the fingerprint only of models that call it, directly or
through another macro.

- Kind: behavior
- Verify: test `mg_req_0644_macro_change_refingerprints_callers_only`

### MG-REQ-0645 · Seeds

Each `seeds/<name>.csv` MUST be declared in `project.toml` as `[seeds.<name>]` with its column
types, and loaded as `raw.seed__<name>`. Its content hash is part of the fingerprint of the
models that read it.

- Kind: behavior
- Verify: test `mg_req_0645_seed_declared_typed_and_hashed`

### MG-REQ-0646 · Seeds stay small

A seed larger than 10 MB MUST produce a validation warning recommending a source.

- Kind: behavior
- Verify: test `mg_req_0646_large_seed_warns`
