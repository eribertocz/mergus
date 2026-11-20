---
id: publish-sequence
title: The publication sequence of stage 6
crate: mergus-runtime
phase: "0.5"
status: draft
depends_on: [run-stages, contracts-manifest, quality-evaluation]
---

# Publication sequence

The manifest is the transaction. Data is written in versions that never change; publishing
is only moving the pointer to a small document that names the current version of each table.

### MG-REQ-0901 · Strict order

Stage 6 MUST run these steps in this order, stopping at the first failure:

1. Run the tests of every candidate (`quality/evaluation.md`).
2. Commit candidates to gold through the adapter and get the data version.
3. Run Menura's integrity checks on that version.
4. Build and commit summary tables.
5. Compute the tokens that changed.
6. Write `catalog_schema` if it changed, then `catalog_state`.
7. Write the manifest.
8. Move the environment pointer by compare-and-swap.

- Kind: behavior
- Verify: test `mg_req_0901_publication_steps_in_order`
- Depends: MG-REQ-0247

### MG-REQ-0902 · All or nothing

A run MUST publish every table of its plan or none. Data committed in step 2 without a
manifest becomes garbage that maintenance removes after the retention.

- Kind: behavior
- Verify: test `mg_req_0902_run_publishes_all_or_nothing`

### MG-REQ-0903 · State never lies

A published `catalog_state` MUST never describe data different from what the executor reads.
When Mergus cannot guarantee it, it MUST withdraw the evidence (omit the affected entries)
rather than publish it.

- Kind: behavior
- Verify: test `mg_req_0903_state_matches_executor_reads`

### MG-REQ-0904 · Rollback window

Gold versions referenced by a manifest inside the rollback window (default 7 days, set per
environment) MUST NOT be expired.

- Kind: behavior
- Verify: test `mg_req_0904_rollback_window_kept`
- Depends: MG-REQ-0288
