---
id: security-protection
title: Isolation, encryption, audit trail and supply chain
crate: mergus-security
phase: "0.6"
status: draft
depends_on: [security-secrets]
---

# Protection

### MG-REQ-1051 · Server-mode sandbox

In server mode, each Polars process with user code MUST run confined with Landlock (no files
outside its temporary directory and Python libraries, no network) and seccomp-bpf (only
needed system calls), with memory, CPU and time limits. Where Landlock is unavailable,
server mode MUST refuse third-party Python unless an equivalent external isolation is
configured. (After the MVP.)

- Kind: behavior
- Verify: test `mg_req_1051_server_sandbox_or_refuse`
- Phase: 1.1

### MG-REQ-1052 · TLS everywhere

Connections to sources, storage and remote catalogs MUST use TLS (rustls). `tls = "insecure"`
MUST be accepted only in the `local` profile.

- Kind: behavior
- Verify: test `mg_req_1052_tls_required_outside_local`

### MG-REQ-1053 · Encryption at rest

Raw MUST be encrypted (MG-REQ-0515). Gold MAY be encrypted with `encrypted = true`. Spill
files MUST be readable only by the running user and deleted at the end of the run.

- Kind: behavior
- Verify: test `mg_req_1053_spill_files_private_and_deleted`

### MG-REQ-1054 · Hash-chained journal

Each journal entry MUST include the hash of the previous one. `mergus audit verify [--env]`
MUST walk the journal and manifest chains and report any deletion, edit or reordering.

- Kind: behavior
- Verify: test `mg_req_1054_audit_verify_detects_tampering`
- Phase: 0.7
- Depends: MG-REQ-0148

### MG-REQ-1055 · Audited actions

`run`, `promote`, `rollback`, `backfill`, `forget`, `secret rotate`, PII decision changes,
`allow` reasons and label removals MUST be journaled with who, when, from where and which
identity.

- Kind: behavior
- Verify: test `mg_req_1055_sensitive_actions_journaled`

### MG-REQ-1056 · Right to be forgotten

`mergus forget --subject <column>=<value>` MUST pseudonymize the value, find affected tables
by lineage, delete the rows in raw, work and gold, publish normally, expire older versions
holding the data, and record an erasure certificate that holds the pseudonym, never the
value. (After the MVP.)

- Kind: behavior
- Verify: test `mg_req_1056_forget_erases_and_certifies`
- Phase: 1.1

### MG-REQ-1057 · Signed releases

Releases MUST be signed keylessly with Sigstore from CI, with SLSA provenance and a CycloneDX
SBOM attached. `cargo-deny` and `cargo-audit` MUST pass on every pull request.

- Kind: process
- Verify: check `release-signing`

### MG-REQ-1058 · Verified extensions

DuckDB extensions and ADBC drivers MUST be loaded only from signed sources or pinned by hash,
never downloaded unverified at run time.

- Kind: behavior
- Verify: test `mg_req_1058_unverified_extension_refused`
