---
id: project-folder-config
title: Folder mergus.toml, model options and precedence
crate: mergus-project
phase: "0.2"
status: draft
depends_on: [project-project-toml]
---

# Folder configuration

A folder under `models/` MAY contain a `mergus.toml` configuring the models of that folder
and its subfolders. A folder whose models need nothing special has no such file.

```toml
[all]                       # every model in this folder and below
checks = ["not_null(*_id)"]
fresh = "6h"

[fact_sales]                # the model models/.../fact_sales.sql
refresh = "incremental"
partition_by = "date:month"
lookback = 3
```

## 1. Model options

| Option | Type | Default | Meaning | Defined in |
|---|---|---|---|---|
| `publish` | string or `false` | from path | Published name, or `false` to stop publishing | `publish/` |
| `refresh` | `full` \| `incremental` \| `history` | `full` | How the table is rebuilt | `model/` |
| `key` | string or list | none | Business key | `model/` |
| `partition_by` | `"<column>:<grain>"` | none | Period column and grain | `model/` |
| `lookback` | integer ≥ 0 | 0 | Periods recomputed each run | `model/refresh.md` |
| `by`, `deletes` | see history | — | Change detection and deletes of `history` | `model/history.md` |
| `fresh` | duration | none | Freshness target | `run/` |
| `checks` | list of strings | `[]` | Tests | `quality/` |
| `materialize` | boolean | `false` | Store an intermediate table | `model/refresh.md` |
| `nondeterministic` | list of strings | `[]` | Declared non-deterministic operations | `model/engines.md` |
| `output` | `"locked"` | none | Output must match `contracts/<model>.json` | `quality/` |
| `pii` | table | none | PII exceptions in gold | `security/` |
| `owner` | string | none | Responsible person or team | MG-REQ-0183 |
| `tags` | list of strings | `[]` | Selection tags | `cli/` |
| `inputs` | table | deduced | Only for inputs that cannot be deduced | `model/` |
| `inherit` | boolean | `true` | `false` ignores `[all]` of parent folders | here |

### MG-REQ-0331 · Sections name existing models

Every section other than `[all]` MUST name a model file of the same folder. Any other section
MUST be an error suggesting the closest model name.

- Kind: behavior
- Verify: test `mg_req_0331_section_for_missing_model_rejected`

### MG-REQ-0332 · Precedence

The effective value of an option MUST be taken from the first source that sets it, in this
order: the model's section; `[all]` of its folder; `[all]` of each parent folder, nearest
first; the folder convention; the default.

- Kind: behavior
- Verify: test `mg_req_0332_option_precedence`

### MG-REQ-0333 · Lists add up

List options (`checks`, `tags`) MUST be the union of every level, in precedence order,
without duplicates. Other options are replaced by the more specific level.

- Kind: behavior
- Verify: test `mg_req_0333_list_options_accumulate`

### MG-REQ-0334 · Opting out of inheritance

With `inherit = false` in a model section, the parent folders' `[all]` MUST be ignored for
that model; its own folder's `[all]` still applies.

- Kind: behavior
- Verify: test `mg_req_0334_inherit_false_skips_parents`

### MG-REQ-0335 · Every value has a known origin

For every effective option of a model, Mergus MUST record which file and line set it, or
that it is a convention or default. `mergus explain <model>` shows it (`project/tools.md`).

- Kind: behavior
- Verify: test `mg_req_0335_origin_recorded_for_each_value`

### MG-REQ-0366 · Option values in phase 0.2

In phase 0.2 the folder schema MUST accept exactly these options and values; `pii`, `by`,
`deletes` and `inputs` join it with the phases that define their content:

1. MG-REQ-0366.1: `publish`: `false`, or a string `gold.<schema>.<table>`, both names of
   MG-REQ-0305;
2. MG-REQ-0366.2: `refresh`: `full`, `incremental` or `history`;
3. MG-REQ-0366.3: `key`: a name of MG-REQ-0305, or a non-empty list of them;
4. MG-REQ-0366.4: `partition_by`: `<column>:<grain>`, the column a name of MG-REQ-0305 and the
   grain one of `day`, `week`, `month`, `quarter` and `year`;
5. MG-REQ-0366.5: `lookback`: an integer from 0; `fresh`: a duration, a positive integer
   followed by `m`, `h` or `d`, such as `6h`;
6. MG-REQ-0366.6: `checks` and `nondeterministic`: lists of non-empty strings; `tags`: a list
   of names of MG-REQ-0305; `owner`: a non-empty string; `materialize`: a boolean; `output`:
   `"locked"`;
7. MG-REQ-0366.7: `inherit`: a boolean, in a model table only, not in `[all]`.

- Kind: behavior
- Verify: test `mg_req_0366_option_values_validated`
- Decision: 0019

### MG-REQ-0367 · Shape of an origin

The origin of MG-REQ-0335 MUST be one of: `file` and `line`, the path of the `mergus.toml`
relative to the project root with `/` and the 1-based line of the key; `convention`, for a
value taken from the folder (`publish` of MG-REQ-0304); or `default`, for the default of
section 1. A list option has one origin per item.

- Kind: behavior
- Verify: test `mg_req_0367_origin_shape`
- Depends: MG-REQ-0335
- Decision: 0019
