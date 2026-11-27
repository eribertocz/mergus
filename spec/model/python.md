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
same isolation as execution:

1. MG-REQ-0617.1: at the `subprocess` level (MG-REQ-0635.3), the call runs in a Python child
   process started through the Python port. Its environment is only the allowlisted one of
   MG-REQ-0006, and its working directory is the step's own directory under the run's work
   directory. The child imports the model file by its path, with the project's `lib`
   directory first on `sys.path`, and calls the step with one keyword argument per
   parameter.
2. MG-REQ-0617.2: for planning, each argument is an empty LazyFrame with its input's schema.
   The child writes the schema of the returned LazyFrame as a zero-row Arrow IPC file in its
   working directory, and Mergus reads the output schema from that file.
3. MG-REQ-0617.3: for execution, Mergus writes each input as an Arrow IPC file in that
   directory, and the child passes it as `pl.scan_ipc(<file>)`. The child collects the
   result once and writes it as an Arrow IPC file, which Mergus reads.
4. MG-REQ-0617.4: an exception in the call, or a return value that is not a LazyFrame, is a
   typed error with the next free `MG-06xx` code and exit code 1. It names the model, the step,
   and the exception's type and message. A child that cannot import `polars` gets its own
   next free `MG-06xx` code, with the fix "install polars in the project's Python".
5. MG-REQ-0617.5: the step's own failures, an exception in the call and an exception while
   the result is collected, share the code of MG-REQ-0617.4. A model file that cannot be
   loaded, because importing it raises or the step's function is not in the module, is a
   different error with its own next free `MG-06xx` code and exit code 1. It names the model,
   the file, and the exception's type and message, and its fix is to fix the file.

- Kind: behavior
- Verify: test `mg_req_0617_polars_planned_with_empty_frames`

### MG-REQ-0618 · Shared code lives in lib

Reusable Python MUST live in `lib/*.py` and be imported normally. Mergus MUST include the
imported `lib` modules in the fingerprint of the models that import them.

- Kind: behavior
- Verify: test `mg_req_0618_lib_changes_refingerprint_importers`

### MG-REQ-0619 · Testable as plain functions

A Polars step MUST be callable from a normal Python test with a hand-made LazyFrame, without
Mergus running. `mergus preview <model> --step <step>` shows the output of any step.:

1. MG-REQ-0619.1: Mergus adds nothing to the model's module, and the step needs no Mergus
   package. Imported from a test and called with LazyFrames built by hand, the step returns
   the same LazyFrame as when Mergus runs it on the same data.

- Kind: behavior
- Verify: test `mg_req_0619_step_callable_in_pytest`
