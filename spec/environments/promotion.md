---
id: environments-promotion
title: diff, promote and rollback
crate: mergus-runtime
phase: "0.7"
status: draft
depends_on: [environments-environments]
---

# Promotion

### MG-REQ-1221 · diff

`mergus diff <env> <env>` MUST show changed models with their classification, schema changes,
the schema gate result, row and aggregate differences of affected tables, test results, and
the estimated cost of rebuilding versus reusing.

- Kind: behavior
- Verify: test `mg_req_1221_diff_sections`

### MG-REQ-1222 · Promote by compare-and-swap

`mergus promote <env> --to prod` MUST move the prod pointer only if prod is still at the
manifest the promotion was validated against; otherwise it MUST re-plan.

- Kind: behavior
- Verify: test `mg_req_1222_promote_compare_and_swap`

### MG-REQ-1223 · Instant when prod did not move

When prod data did not advance, promotion MUST point prod to the verified physical tables
and versions without recomputing or copying. When it advanced, only the difference is
recomputed and every gate runs again.

- Kind: behavior
- Verify: test `mg_req_1223_promotion_reuses_verified_tables`

### MG-REQ-1224 · Coordinated model and data

When a change includes a new Ossie model, it MUST be validated in the source environment
against the new model and promoted together with the data.

- Kind: behavior
- Verify: test `mg_req_1224_model_and_data_promoted_together`

### MG-REQ-1225 · Promote only from main

Promotion to prod MUST be accepted only from a commit on the main branch.

- Kind: behavior
- Verify: test `mg_req_1225_promote_requires_main_commit`

### MG-REQ-1226 · Rollback

`mergus rollback <env>` MUST move the pointer to the previous manifest at once. The next
incremental run fills data that arrived in between.

- Kind: behavior
- Verify: test `mg_req_1226_rollback_previous_manifest`

### MG-REQ-1227 · Only prod and staging are scheduled

`dev-*` environments MUST run only on demand.

- Kind: behavior
- Verify: test `mg_req_1227_dev_not_scheduled`
