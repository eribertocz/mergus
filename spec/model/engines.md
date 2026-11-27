---
id: model-engines
title: Segments, boundaries between engines and Polars execution levels
crate: mergus-plan
phase: "0.4"
status: draft
depends_on: [model-python]
---

# Engines

Cost lives at the boundaries between engines, not in the steps. Mergus minimizes boundaries
and makes each one cheap, automatically.

| Polars level | When | How |
|---|---|---|
| native | Only Polars expressions, no Python functions | Plan executed by Polars in Rust, in process, zero copy |
| in-process | Python functions inside the step, local mode | Python in the Mergus process, sharing Arrow memory |
| isolated | Server mode or untrusted code | Separate process, Arrow over shared memory |

### MG-REQ-0631 · Fused segments

Consecutive steps of the same engine MUST be fused into one segment: one DuckDB query (a CTE
chain) or one chained LazyFrame:

1. MG-REQ-0631.1: steps are fused only inside one model.
2. MG-REQ-0631.2: a `.sql` model is one step, named like the model, and so one segment
   `{index: 0, engine: duckdb, steps: [<model name>]}`. Every model in a plan has at least one
   segment.

- Kind: behavior
- Verify: test `mg_req_0631_consecutive_steps_fused`

### MG-REQ-0632 · Column pruning across boundaries

At each boundary Mergus MUST transfer only the columns the next segment uses.

- Kind: behavior
- Verify: test `mg_req_0632_boundary_transfers_used_columns_only`

### MG-REQ-0633 · Adaptive transport

A boundary MUST use Arrow in shared memory when it fits the memory budget, otherwise Parquet
in the work zone read lazily:

1. MG-REQ-0633.1: the plan chooses the transport. The caller gives an optional row estimate
   for each upstream model. The boundary's size is that estimate times the sum of its column
   widths: the fixed width of each fixed-width type, and 64 bytes for each variable-width type
   (`VARCHAR`, `BLOB`, nested types).
2. MG-REQ-0633.2: the boundary uses `arrow` when that size is at most the budget, 1 GiB until
   `[resources]` exists. It uses `parquet` when the size is larger or no estimate is given.

- Kind: behavior
- Verify: test `mg_req_0633_transport_falls_back_to_parquet`

### MG-REQ-0634 · Type passport

Before execution, the schema at every boundary MUST be checked. A conversion that loses
information MUST block the run (MG-REQ-0180). Types are checked again at execution:

1. MG-REQ-0634.1: the caller gives the planner each step's output columns, with their DuckDB
   type names as `DESCRIBE` writes them. For a Polars step, these are the types of its empty
   frame (MG-REQ-0617), mapped to DuckDB names.
2. MG-REQ-0634.2: these types lose information across a DuckDB to Polars boundary, and only
   these: `HUGEINT`, `UHUGEINT`, `UNION`, `BIT`, `INTERVAL`, `DECIMAL` with more than 38 digits,
   and any type name the planner does not know. Every other conversion is lossless.
3. MG-REQ-0634.3: a lossy column is an error of the `Type passport` gate (MG-REQ-0720). It
   names the model, the step, the column and its type. Its code is the next free `MG-07xx`,
   with exit code 1.

- Kind: behavior
- Verify: test `mg_req_0634_lossy_boundary_blocks`

### MG-REQ-0635 · Polars level falls back

Mergus MUST pick the highest Polars level a step qualifies for and fall back to the next one
on any mismatch (for example Polars versions), never losing correctness. The native level is
an optimization, never a requirement:

1. MG-REQ-0635.1: the levels, from highest to lowest, are `native`, `in_process` and
   `subprocess`. A segment takes the lowest level that any of its steps qualifies for.
   Fusion never splits a segment because of a level.
2. MG-REQ-0635.2: `polars_reason` is `qualified` when the segment runs at the highest level,
   `version_mismatch` when a higher level was refused because the Polars versions differ, and
   `not_available` when the higher levels are not in this build.
3. MG-REQ-0635.3: until a phase adds them, `native` and `in_process` are not available. Every
   Polars segment is `subprocess` with reason `not_available`. When `subprocess` itself fails,
   for example because Polars is missing, the step fails at execution with a typed error.

- Kind: behavior
- Verify: test `mg_req_0635_polars_level_fallback`

### MG-REQ-0636 · Isolation in server mode

In server mode, every Polars step with Python code MUST run isolated, with no access to
credentials, the network or files outside its work directory:

1. MG-REQ-0636.1: until a phase adds server mode, the isolation of every Polars step is the
   one of MG-REQ-0617.1. No secret value is in the child's environment, its arguments or the
   files of its working directory, and the step's working directory is fresh for each step
   and each run. The network and file system limits apply when server mode is added.

- Kind: behavior
- Verify: test `mg_req_0636_server_mode_isolates_python`
- Depends: MG-REQ-0006

### MG-REQ-0637 · Non-determinism detector

For published models, Mergus MUST flag operations that can return different results on the
same data (DuckDB: `LIMIT` without `ORDER BY`, `ANY_VALUE`, `random()`, `now()`; Polars:
`unique()` without `keep`, `first()` without a prior sort) and block publication unless the
model declares them in `nondeterministic = [...]`:

1. MG-REQ-0637.1: the names declared in `nondeterministic` are `limit_without_order`,
   `any_value`, `random`, `now`, `unique_without_keep` and `first_without_sort`.
2. MG-REQ-0637.2: SQL is checked on the tree of the `SqlParser` port, never on the text:
   - a `LIMIT` in a `SELECT` with no `ORDER BY` at the same level;
   - a call of `any_value`, `random` or `now`, matched by function name without case.
3. MG-REQ-0637.3: Polars is checked on the tree of the `Python` port:
   - a `.unique(` call without a `keep` keyword;
   - a `.first(` call with no `.sort(` earlier in the same method chain.
4. MG-REQ-0637.4: an operation that a published model does not declare is an error of the
   `Determinism` gate (MG-REQ-0720). It names the model, the step, the name and the location.
   Its code is the next free `MG-07xx`, with exit code 1.

- Kind: behavior
- Verify: test `mg_req_0637_nondeterminism_flagged`

### MG-REQ-0638 · Row order never matters

Fingerprints, tokens and hashes MUST NOT depend on row order.

- Kind: behavior
- Verify: test `mg_req_0638_hashes_independent_of_row_order`
