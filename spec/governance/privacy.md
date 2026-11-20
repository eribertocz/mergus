---
id: governance-privacy
title: Zero telemetry, network declaration and bug reports
crate: none
phase: "0.9"
status: draft
depends_on: [kernel]
---

# Privacy

MG-REQ-0010 forbids sending data to undeclared destinations. This file makes it verifiable.

### MG-REQ-1761 · Network only on request

Mergus MUST connect only to configured sources and destinations, configured telemetry or
lineage endpoints, and download hosts (or `MERGUS_MIRROR`) when a version or adapter is
needed or `self update` is run.

- Kind: behavior
- Verify: test `mg_req_1761_only_declared_hosts_contacted`

### MG-REQ-1762 · doctor --network

`mergus doctor --network` MUST list every destination the project will contact. The server
mode network allowlist MUST be derived from this list.

- Kind: behavior
- Verify: test `mg_req_1762_doctor_lists_destinations`

### MG-REQ-1763 · bug-report is local

`mergus bug-report` MUST build a local package with version, system, error code and plan,
without data, secrets or PII, and MUST NOT send it anywhere.

- Kind: behavior
- Verify: test `mg_req_1763_bug_report_local_and_clean`

### MG-REQ-1764 · DO_NOT_TRACK honoured

Mergus MUST honour `DO_NOT_TRACK` even though it sends nothing to disable.

- Kind: behavior
- Verify: test `mg_req_1764_do_not_track_accepted`

### MG-REQ-1765 · PRIVACY.md

The repository MUST contain `PRIVACY.md` with the zero-telemetry promise; changing it
requires a public RFC.

- Kind: process
- Verify: check `governance-files`
