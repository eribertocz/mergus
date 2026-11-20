---
id: architecture-invariants
title: Named invariants checked by the simulator and at run time
crate: mergus-runtime
phase: "0.1"
status: draft
depends_on: [architecture-ports]
---

# Invariants

Each invariant is a promise Mergus makes. The simulator (`architecture/testing.md`) checks
every invariant after every step. Invariants marked **runtime** are also asserted in
production; a violation stops the run without publishing and reports an internal error.

### MG-REQ-0140 · INV-01 The pointer only advances to complete manifests

An environment pointer MUST move only to a manifest that is complete, valid against its
schema, and whose data files are all durably committed. (runtime)

- Kind: behavior
- Verify: test `mg_req_0140_pointer_advances_only_to_complete_manifest`
- Phase: 0.5

### MG-REQ-0141 · INV-02 One writer identity for gold

Gold tables MUST be written only through the writer identity of `security/identities.md`.

- Kind: behavior
- Verify: test `mg_req_0141_gold_written_only_by_writer_identity`
- Phase: 0.5

### MG-REQ-0142 · INV-03 Tokens follow changes

A period whose data did not change MUST keep its data token; a period whose data changed
MUST receive a new token.

- Kind: behavior
- Verify: test `mg_req_0142_tokens_change_only_with_data`
- Phase: 0.5

### MG-REQ-0143 · INV-04 Resuming never duplicates or loses rows

Resuming an interrupted run MUST produce exactly the rows an uninterrupted run would.

- Kind: behavior
- Verify: test `mg_req_0143_resume_is_exactly_once`
- Phase: 0.3

### MG-REQ-0144 · INV-05 Pinned readers see one version

A reader pinned to a manifest MUST see only data of that manifest's version.

- Kind: behavior
- Verify: test `mg_req_0144_pinned_reader_sees_one_version`
- Phase: 0.5

### MG-REQ-0145 · INV-06 Secrets never leak

No secret value MUST appear in logs, errors, the journal, events, manifests, catalog
documents or ops tables. (runtime)

- Kind: behavior
- Verify: test `mg_req_0145_no_secret_in_any_output`
- Phase: 0.6

### MG-REQ-0146 · INV-07 Unclassified PII never reaches raw

A column labelled `pii` without a decision (`block`, `pseudonymize` or `allow`) MUST NOT be
written to raw. (runtime)

- Kind: behavior
- Verify: test `mg_req_0146_unclassified_pii_blocked`
- Phase: 0.6

### MG-REQ-0147 · INV-08 Exactly one concurrent writer publishes

When two runs try to publish the same environment concurrently, exactly one MUST publish;
the other MUST end with exit code 4 and no visible effect.

- Kind: behavior
- Verify: test `mg_req_0147_one_concurrent_writer_publishes`
- Phase: 0.5

### MG-REQ-0148 · INV-09 Hash chains verify

The hash chains of the journal and of manifests MUST always verify with
`mergus audit verify`.

- Kind: behavior
- Verify: test `mg_req_0148_hash_chains_verify`
- Phase: 0.7

### MG-REQ-0149 · INV-10 Same input, same output

The same project, the same input data and the same version MUST produce identical tokens,
hashes, manifests and catalog documents.

- Kind: behavior
- Verify: test `mg_req_0149_identical_outputs_for_identical_inputs`
- Phase: 0.5
- Depends: MG-REQ-0005

### MG-REQ-0150 · New promises become invariants

A new guarantee of the same kind (visibility, exactly-once, isolation, secrecy, integrity)
MUST be added here as an invariant with its simulator check before it is implemented.

- Kind: process
- Verify: review `new-invariant`
