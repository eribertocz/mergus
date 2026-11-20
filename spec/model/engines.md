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
chain) or one chained LazyFrame.

- Kind: behavior
- Verify: test `mg_req_0631_consecutive_steps_fused`

### MG-REQ-0632 · Column pruning across boundaries

At each boundary Mergus MUST transfer only the columns the next segment uses.

- Kind: behavior
- Verify: test `mg_req_0632_boundary_transfers_used_columns_only`

### MG-REQ-0633 · Adaptive transport

A boundary MUST use Arrow in shared memory when it fits the memory budget, otherwise Parquet
in the work zone read lazily.

- Kind: behavior
- Verify: test `mg_req_0633_transport_falls_back_to_parquet`

### MG-REQ-0634 · Type passport

Before execution, the schema at every boundary MUST be checked. A conversion that loses
information MUST block the run (MG-REQ-0180). Types are checked again at execution.

- Kind: behavior
- Verify: test `mg_req_0634_lossy_boundary_blocks`

### MG-REQ-0635 · Polars level falls back

Mergus MUST pick the highest Polars level a step qualifies for and fall back to the next one
on any mismatch (for example Polars versions), never losing correctness. The native level is
an optimization, never a requirement.

- Kind: behavior
- Verify: test `mg_req_0635_polars_level_fallback`

### MG-REQ-0636 · Isolation in server mode

In server mode, every Polars step with Python code MUST run isolated, with no access to
credentials, the network or files outside its work directory.

- Kind: behavior
- Verify: test `mg_req_0636_server_mode_isolates_python`
- Depends: MG-REQ-0006

### MG-REQ-0637 · Non-determinism detector

For published models, Mergus MUST flag operations that can return different results on the
same data (DuckDB: `LIMIT` without `ORDER BY`, `ANY_VALUE`, `random()`, `now()`; Polars:
`unique()` without `keep`, `first()` without a prior sort) and block publication unless the
model declares them in `nondeterministic = [...]`.

- Kind: behavior
- Verify: test `mg_req_0637_nondeterminism_flagged`

### MG-REQ-0638 · Row order never matters

Fingerprints, tokens and hashes MUST NOT depend on row order.

- Kind: behavior
- Verify: test `mg_req_0638_hashes_independent_of_row_order`
