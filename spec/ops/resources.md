---
id: ops-resources
title: Resource governor, forecasts and memory safety
crate: mergus-runtime
phase: "0.9"
status: draft
depends_on: [run-plan]
---

# Resources

```toml
[resources]
memory = "70%"
temp_dir = "/fast-ssd/mergus"
temp_max = "100GB"
threads = "auto"
```

### MG-REQ-1501 · One budget, one governor

A governor MUST split `memory` and `threads` between DuckDB, Polars and parallel models so
that their sum never exceeds the budget.

- Kind: behavior
- Verify: test `mg_req_1501_governor_respects_budget`

### MG-REQ-1502 · Heavy operations go where they spill

When a Polars step joins or aggregates an input estimated larger than its memory share, the
plan MUST warn and suggest writing that step in SQL.

- Kind: behavior
- Verify: test `mg_req_1502_large_polars_join_warned`

### MG-REQ-1503 · Forecast

`mergus explain --resources` MUST estimate memory, temporary disk and time per model from
run history and input sizes.

- Kind: behavior
- Verify: test `mg_req_1503_resource_forecast`

### MG-REQ-1504 · Retry on out of memory

A model that fails for lack of memory MUST be retried once automatically with less
parallelism and more spill, recorded with the suggested setting.

- Kind: behavior
- Verify: test `mg_req_1504_oom_retry_with_less_parallelism`

### MG-REQ-1505 · Per-model hints

A model MAY set `memory = "<size>"` and `run_alone = true` in its folder `mergus.toml`; the
governor MUST honour them.

- Kind: behavior
- Verify: test `mg_req_1505_model_resource_hints`
