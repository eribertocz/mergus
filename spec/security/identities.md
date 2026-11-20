---
id: security-identities
title: The four identities and least privilege
crate: mergus-security
phase: "0.6"
status: draft
depends_on: [security-secrets, storage-zones]
---

# Identities

| Identity | Sources | raw | work | gold | catalog | ops |
|---|---|---|---|---|---|---|
| Source reader (extraction) | read | write | — | — | — | write |
| Writer (transform, publish) | — | read | read, write | write (only door) | write | write |
| Maintenance | — | expire | clean | clean unreferenced | read | write |
| BI reader / executor | — | never | — | read pinned | read | — |

### MG-REQ-1011 · Each stage uses its identity

Extraction MUST use the source reader identity, transformation and publication the writer,
maintenance the maintenance identity. In the `local` profile they MAY be the same account.

- Kind: behavior
- Verify: test `mg_req_1011_stage_uses_its_identity`

### MG-REQ-1012 · Security doctor

`mergus doctor --security` MUST check that each identity has only the permissions of this
table and report every excess, for example a BI reader that can read raw.

- Kind: behavior
- Verify: test `mg_req_1012_doctor_reports_excess_permissions`

### MG-REQ-1013 · Read-only source connections

Source connections MUST be opened read-only whenever the driver supports it.

- Kind: behavior
- Verify: test `mg_req_1013_source_connection_read_only`

### MG-REQ-1014 · Production promotion identity

`mergus promote --to prod` MUST require the production writer identity; in CI it MUST come
from OIDC, never from a stored key.

- Kind: behavior
- Verify: test `mg_req_1014_promote_requires_prod_identity`

### MG-REQ-1015 · Row security belongs to the semantic layer

Mergus MUST NOT implement row-level security or dynamic masking for consumers; it delivers
minimized data and trustworthy labels, and Menura or the BI tool enforce access.

- Kind: process
- Verify: review `no-row-security`
