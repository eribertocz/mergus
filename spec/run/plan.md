---
id: run-plan
title: Fingerprints and the plan artifact
crate: mergus-plan
phase: "0.2"
status: draft
depends_on: [run-stages]
---

# Plan

### MG-REQ-0711 · Fingerprint contents

A model's fingerprint MUST be the SHA-256 of the canonical JSON of: its code and effective
configuration; the versions of all its inputs (raw tokens, fingerprints of upstream models,
versions of connected sources, seed hashes, used macros and `lib` modules); the run
parameters it uses; the versions of DuckDB, Polars and pinned Python packages; and the
version of its output contract.

- Kind: behavior
- Verify: test `mg_req_0711_fingerprint_changes_with_each_input`

### MG-REQ-0712 · Skip unchanged work

A model whose fingerprint equals that of an available result (in gold or work) MUST be
skipped and its result reused.

- Kind: behavior
- Verify: test `mg_req_0712_unchanged_model_skipped`

### MG-REQ-0713 · Plan decisions

The plan MUST record, per model: skip or compute (full or which periods), segments, boundary
columns and transport, Polars level, write strategy, parallel group, and what the publication
will contain; each decision with its reason.

- Kind: behavior
- Verify: test `mg_req_0713_plan_records_decisions_and_reasons`

### MG-REQ-0714 · Plan is an immutable artifact

The plan MUST be an immutable JSON document identified by its hash (`plan_id`), valid against
the `mergus_plan` contract (MG-REQ-0742). `mergus plan` writes it without executing.

- Kind: behavior
- Verify: test `mg_req_0714_plan_is_hashed_artifact`

### MG-REQ-0715 · Plan is deterministic

The same project and the same input versions MUST produce a byte-identical plan.

- Kind: behavior
- Verify: test `mg_req_0715_plan_byte_identical`
- Depends: MG-REQ-0005

### MG-REQ-0716 · Parallelism within the budget

Independent branches MUST run in parallel only while the resource budget allows; otherwise
in series.

- Kind: behavior
- Verify: test `mg_req_0716_parallelism_respects_budget`

### MG-REQ-0717 · Selection never runs on stale data silently

When a selection leaves out an upstream model whose result is stale, the plan MUST say so
and propose the selector that includes it. It MUST NOT run on stale inputs without stating it.

- Kind: behavior
- Verify: test `mg_req_0717_stale_upstream_reported`

## Phase 0.2

Before storage (0.3) and execution (0.4), the plan holds only the decisions below. Segments,
boundaries, Polars levels, write strategies and publication contents of MG-REQ-0713 arrive as
optional fields in later phases (a minor contract change).

### MG-REQ-0742 · The plan contract

The plan contract MUST be named `mergus_plan`, live in `schemas/mergus_plan/v1/` and start at
version `1.0.0`. A plan has `contract`, `contract_version`, `plan_id`, `mergus_version`,
`environment` (MG-REQ-0296.5), `max_parallel`, `selected` (model names), `steps` and
`stale_upstream`. A step has `model`, `model_id` (MG-REQ-0368), `fingerprint`, `inputs`
(names as written in code, sorted), `action` (`compute` or `skip`), `reason`, `refresh`
(`full` in 0.2) and `parallel_group`. Every list is sorted by byte order unless stated.

- Kind: behavior
- Verify: test `mg_req_0742_plan_contract_fields`
- Phase: 0.2
- Decision: 0026

### MG-REQ-0743 · plan_id and fingerprints

`plan_id` MUST be the lowercase hexadecimal SHA-256 of the canonical JSON (MG-REQ-0208) of the
plan without `plan_id`. A fingerprint (MG-REQ-0711) MUST be the same encoding of the object
`code` (the model file's text with CRLF turned into LF), `config` (effective option values,
without origins), `inputs` (input name to the upstream fingerprint, or to the version the
caller gives, or `null`), `parameters`, `engines` (name to version, as the caller gives) and
`output_contract` (`null` until locked outputs exist). Nothing else MAY enter either hash.

- Kind: behavior
- Verify: test `mg_req_0743_plan_id_and_fingerprint_inputs`
- Phase: 0.2
- Decision: 0026

### MG-REQ-0744 · Decisions, results and budget

1. MG-REQ-0744.1: The planner MUST receive the fingerprints of available results and
   `max_parallel` (default 4) from its caller; in 0.2 the CLI gives no results.
2. MG-REQ-0744.2: A step is `skip` with reason `result_available` when its fingerprint is
   available, otherwise `compute` with reason `no_result`.
3. MG-REQ-0744.3: Steps of the same depth (longest chain of selected upstream models) are cut,
   in name order, into groups of at most `max_parallel`; groups are numbered from 1 by depth
   and then by order, and `steps` is sorted by group and name.

- Kind: behavior
- Verify: test `mg_req_0744_decisions_results_and_groups`
- Phase: 0.2
- Decision: 0026

### MG-REQ-0745 · Stale upstream in the plan

For MG-REQ-0717, each model that is not selected, is upstream of a selected model and would be
`compute`, MUST appear in `stale_upstream` as `model` and `include`, the selector
`+<model>` of the first selected model downstream of it, in name order.

- Kind: behavior
- Verify: test `mg_req_0745_stale_upstream_listed`
- Phase: 0.2
- Decision: 0026

### MG-REQ-0746 · mergus plan

`mergus plan [<selection>]` MUST write the plan to `.mergus/plans/<plan_id>.json` under the
project root, leave an existing file with that name untouched, emit `plan_ready`, and execute
nothing. `.mergus/` holds local state and is never committed.

- Kind: behavior
- Verify: test `mg_req_0746_plan_command_writes_artifact`
- Phase: 0.2
- Decision: 0026
