---
id: quality-tests
title: Data tests, actions and output contracts
crate: mergus-quality
phase: "0.5"
status: draft
depends_on: [project-folder-config]
---

# Tests

Tests are written in `checks` of the folder `mergus.toml`. Quality is part of the contract:
a published table promises its columns and types and the rules its data meets.

| Test | Checks | Dimension |
|---|---|---|
| `not_null(cols)` | No nulls; `*_id` patterns allowed | Completeness |
| `unique(cols)` | No duplicates over the columns | Uniqueness |
| `accepted_values(col, [values])` | Only listed values | Validity |
| `range(col, min=, max=)` | Value between bounds | Validity |
| `relationship(col, model.col)` | Every value exists in another model | Consistency |
| `expr('<sql boolean>')` | Condition true for every row | Validity |
| `row_count_change(max=<pct>)` | Row count vs previous publication | Volume |
| `freshness(col, max=<duration>)` | Newest value is recent | Freshness |

| Prefix | Action |
|---|---|
| none or `block` | The new version is not published (default) |
| `warn` | Published with a visible warning |
| `quarantine` | After the MVP |

### MG-REQ-0801 · Test syntax

Each `checks` entry MUST parse as `[block|warn] <test>(<arguments>)` with the tests of this
table. An unknown test or a malformed entry MUST be a validation error with the closest test.

- Kind: behavior
- Verify: test `mg_req_0801_check_syntax`

### MG-REQ-0802 · Locked output contract

A model with `output = "locked"` MUST produce exactly the columns, types and nullability of
`contracts/<model>.json`; a difference blocks in stage 4 with the column-by-column diff.

- Kind: behavior
- Verify: test `mg_req_0802_locked_output_enforced`

### MG-REQ-0803 · Block by default

A failing test without prefix MUST block the publication of the whole run (exit code 2). A
`warn` test MUST publish and attach the warning to the table.

- Kind: behavior
- Verify: test `mg_req_0803_block_default_warn_publishes`

### MG-REQ-0804 · Tests use run parameters, not clocks

Tests MUST NOT use non-deterministic functions (`current_date`, `now()`); they use run
parameters such as `$run_date`.

- Kind: behavior
- Verify: test `mg_req_0804_tests_reject_clock_functions`

### MG-REQ-0805 · Strict integrity

With `strict_integrity = true` in `project.toml`, a failed Menura integrity check MUST also
block publication. By default it does not.

- Kind: behavior
- Verify: test `mg_req_0805_strict_integrity_blocks`
