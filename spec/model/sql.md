---
id: model-sql
title: SQL models
crate: mergus-project
phase: "0.4"
status: draft
depends_on: [project-layout]
---

# SQL models

A `.sql` model is one DuckDB `SELECT` (CTEs allowed) that can be pasted into DuckDB as is.

```sql
-- models/gold/sales/fact_sales.sql
SELECT s.*, c.segment
FROM clean_sales s
JOIN clean_customers c USING (customer_id)
```

### MG-REQ-0601 · One query per file

A `.sql` model MUST contain exactly one query statement (`SELECT`, `WITH … SELECT`,
`FROM …`). Any other statement MUST be a validation error.

- Kind: behavior
- Verify: test `mg_req_0601_sql_model_is_one_query`

### MG-REQ-0602 · Inputs deduced from the query

The inputs of a SQL model MUST be deduced by DuckDB's parser from the relations it names.
A CTE named like a model shadows the model inside that query.

- Kind: behavior
- Verify: test `mg_req_0602_inputs_deduced_and_cte_shadows`

### MG-REQ-0603 · Run parameters are bound

Values that change per run MUST be written as named parameters (`$run_date`) and bound at
execution, never substituted as text.

- Kind: behavior
- Verify: test `mg_req_0603_parameters_bound_not_substituted`
- Depends: MG-REQ-0007

### MG-REQ-0604 · Types checked without data

Mergus MUST obtain the output schema of every SQL step by preparing it in DuckDB without
executing it.

- Kind: behavior
- Verify: test `mg_req_0604_sql_schema_by_prepare`
