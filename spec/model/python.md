---
id: model-python
title: Python models without decorators
crate: mergus-project
phase: "0.4"
status: draft
depends_on: [project-layout, model-sql]
---

# Python models

```python
# models/gold/sales/sales_by_segment.py
import polars as pl

def joined(fact_sales, exchange_rates):
    return """
        SELECT f.*, r.rate FROM fact_sales f JOIN exchange_rates r USING (date)
    """

def in_usd(joined):
    return joined.with_columns((pl.col("amount") * pl.col("rate")).alias("amount_usd"))

def sales_by_segment(in_usd):
    return "SELECT segment, SUM(amount_usd) AS total_usd FROM in_usd GROUP BY ALL"
```

### MG-REQ-0611 · Read without importing

Mergus MUST read a `.py` model by parsing its syntax tree, without importing or executing
it, to find its functions, parameters, return kinds and literal SQL.

- Kind: behavior
- Verify: test `mg_req_0611_python_parsed_not_imported`

### MG-REQ-0612 · Every public function is a step

Every top-level function whose name does not start with `_` MUST be a step. Functions
starting with `_` are private helpers.

- Kind: behavior
- Verify: test `mg_req_0612_public_functions_are_steps`

### MG-REQ-0613 · The return decides the engine

A step that returns a string literal MUST be a DuckDB step; a step that returns a Polars
LazyFrame MUST be a Polars step. A step that returns anything else, or SQL built from
variables or f-strings, MUST be a validation error.

- Kind: behavior
- Verify: test `mg_req_0613_return_kind_selects_engine`

### MG-REQ-0614 · Parameter names wire the steps

Each parameter name MUST resolve to another step of the same file, a model, a raw table or a
seed, in that order of precedence. Raw tables and seeds are named without the schema:
`erp__sales` for `raw.erp__sales`, `seed__countries` for `raw.seed__countries`. An unresolved name is an error naming the closest match.

- Kind: behavior
- Verify: test `mg_req_0614_parameters_resolve_inputs`

### MG-REQ-0615 · The function named like the file is the result

The step whose name equals the file stem MUST be the model's result, wherever it is written
in the file. A file without it MUST be an error stating the missing function name.

- Kind: behavior
- Verify: test `mg_req_0615_result_function_required`

### MG-REQ-0616 · Steps return recipes, never data

A Polars step MUST NOT call `.collect()`, `.fetch()` or convert to eager data. Mergus MUST
reject it with the fix ("return the LazyFrame").

- Kind: behavior
- Verify: test `mg_req_0616_collect_rejected`

### MG-REQ-0617 · Polars schema from empty frames

Mergus MUST obtain a Polars step's plan and output schema by calling the function once with
empty LazyFrames that carry the input schemas. In server mode this call runs inside the
same isolation as execution.

- Kind: behavior
- Verify: test `mg_req_0617_polars_planned_with_empty_frames`

### MG-REQ-0618 · Shared code lives in lib

Reusable Python MUST live in `lib/*.py` and be imported normally. Mergus MUST include the
imported `lib` modules in the fingerprint of the models that import them.

- Kind: behavior
- Verify: test `mg_req_0618_lib_changes_refingerprint_importers`

### MG-REQ-0619 · Testable as plain functions

A Polars step MUST be callable from a normal Python test with a hand-made LazyFrame, without
Mergus running. `mergus preview <model> --step <step>` shows the output of any step.

- Kind: behavior
- Verify: test `mg_req_0619_step_callable_in_pytest`
