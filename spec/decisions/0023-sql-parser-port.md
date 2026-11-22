# 0023 · SQL parsed by DuckDB through a port, from phase 0.2

- Status: accepted
- Date: 2026-11-22
- Answers: open questions of #80

## Context

MG-REQ-0307 and MG-REQ-0308 are due in phase 0.2, but the parser of MG-REQ-0602 arrives with
the SQL models of phase 0.4. `mergus-project` is core and cannot run DuckDB.

## Decision

- A `SqlParser` port (MG-REQ-0193) wraps DuckDB's `json_serialize_sql`, so the grammar is
  exactly DuckDB's and no second parser exists. The core walks the JSON tree.
- MG-REQ-0373 fixes what a reference resolves to; MG-REQ-0374 lists the rejected constructs,
  with an allowlist of table functions, since new DuckDB table functions often read files.
- Python references (MG-REQ-0614) and macro calls (MG-REQ-0642) stay in phase 0.4, when
  Python and macros are read. Until then a table macro call is rejected by MG-REQ-0374.2.

## Consequences

`mergus-ports` gains a `duckdb` dependency under its `real` feature. The execution engine of
phase 0.4 still disables external access; MG-REQ-0374 is the first line, not the only one.
