---
id: contracts-menura-inputs
title: Documents Mergus obtains from Menura and how it obtains them
crate: mergus-menura
phase: "0.5"
status: draft
depends_on: [contracts-overview, contracts-menura-catalog]
---

# Menura inputs

Mergus never links Menura code. It runs the Menura command-line program as an external
process and reads the documents it prints. The Ossie semantic model is opaque to Mergus.

| Document | Obtained by | Used for |
|---|---|---|
| Validation report | Menura `validate` (model + candidate `catalog_schema`) | Schema gate before publishing |
| `producer_requirements` | Menura's producer-requirements command | Summaries and partitioning to build |
| `integrity_checks` | Menura `integrity-checks` (model + schema + load options + dialect) | Checks run before publishing |

### MG-REQ-0230 · Menura runs as an external process

Mergus MUST obtain every Menura document by running the Menura program as a child process
through the `Process` port. It MUST depend on the command and its inputs, not on flag
spelling, and MUST NOT import Menura crates.

- Kind: behavior
- Verify: check `boundaries`

### MG-REQ-0231 · The semantic model is opaque

Mergus MUST NOT parse or interpret the Ossie model. It stores it as an immutable file
`ossie/<model_hash>.yaml` with pointer `ossie/latest`, and passes it to Menura.

- Kind: behavior
- Verify: check `no-ossie-parser`

### MG-REQ-0232 · Schema gate

Before committing data whose resulting `catalog_schema` differs from the published one,
Mergus MUST run Menura `validate` with the current model and the candidate schema, and MUST
block the publication if the report has any error or any warning `W0*` not present in the
report of the published schema (compared by code and location). A blocked publication changes
no data, schema or state, and the run ends with exit code 5.

- Kind: behavior
- Verify: test `mg_req_0232_schema_gate_blocks_new_errors_and_warnings`

### MG-REQ-0233 · Forcing a breaking schema change

Publishing despite the schema gate MUST require `--force-schema` with `--reason <text>`, and
MUST record the reason, the report and the identity in the journal.

- Kind: behavior
- Verify: test `mg_req_0233_forced_schema_change_is_recorded`

### MG-REQ-0234 · Integrity checks pass on zero rows

Each integrity check MUST run with the pipeline identity and no row policies, and passes when
it returns zero rows. Returned rows are stored for diagnosis up to a configurable limit
(default 100 per check).

- Kind: behavior
- Verify: test `mg_req_0234_integrity_check_passes_on_zero_rows`

### MG-REQ-0235 · A check that cannot run is omitted, not failed

When an integrity check query errors or times out, Mergus MUST omit it from
`catalog_state`, raise an incident and publish the rest. `failed` is reserved for checks that
found violations.

- Kind: behavior
- Verify: test `mg_req_0235_check_error_omitted_and_reported`

### MG-REQ-0236 · Integrity checks are regenerated when stale

Mergus MUST regenerate `integrity_checks` when the model or the schema changes, detected by
`model_hash` and `schema_document_id`.

- Kind: behavior
- Verify: test `mg_req_0236_stale_integrity_checks_regenerated`

### MG-REQ-0237 · Summaries only from producer requirements

Mergus MUST build summary tables only as declared in `producer_requirements`, and only when
the pinned Menura version uses them (0.6.0 or later). The user does not write pipelines for
summaries.

- Kind: behavior
- Verify: test `mg_req_0237_summaries_built_from_requirements_only`

### MG-REQ-0238 · Summaries are read only by the executor

Summary tables MUST be readable only by the executor identity.

- Kind: behavior
- Verify: test `mg_req_0238_summary_tables_executor_only`
