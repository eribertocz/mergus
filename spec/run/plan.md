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
`schemas/plan.schema.json`. `mergus plan` writes it without executing.

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
