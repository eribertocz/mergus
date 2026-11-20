---
id: security-pii
title: Personal data handled at the door, detection and labels
crate: mergus-security
phase: "0.6"
status: draft
depends_on: [security-secrets, extract-sources]
---

# Personal data

```toml
# sources/erp.toml
[pii]
"customers.email"    = "pseudonymize"
"customers.phone"    = "block"
"customers.notes"    = { allow = true, reason = "free-text support notes, legal basis X" }
```

| Decision | Effect |
|---|---|
| `block` | The column is excluded from the extraction query; it never leaves the source |
| `pseudonymize` | The value is replaced by hex `HMAC-SHA256(environment key, normalized value)` |
| `allow` | Passes unchanged with label `pii`; requires `reason`; audited |

### MG-REQ-1031 · Applied at extraction

PII decisions MUST be applied during extraction, from the first load, so that blocked and
pseudonymized values never reach raw.

- Kind: behavior
- Verify: test `mg_req_1031_pii_applied_before_raw`
- Depends: MG-REQ-0146

### MG-REQ-1032 · Detector

New and first-seen columns MUST be checked by name (lists in English, Spanish and Portuguese:
email, correo, dni, ssn, phone, telefono, iban, name, nombre, address, direccion and similar)
and by patterns on a small in-memory sample never stored (emails, cards with Luhn, IBAN,
phones). The detector is local and deterministic, without AI or external services.

- Kind: behavior
- Verify: test `mg_req_1032_detector_names_and_patterns`

### MG-REQ-1033 · Keyed pseudonyms

Pseudonyms MUST use a per-environment key from the secret chain. Values MUST be normalized
before hashing (trimmed; emails lowercase; document numbers digits only) so that the same
value yields the same pseudonym across sources of one environment.

- Kind: behavior
- Verify: test `mg_req_1033_pseudonym_keyed_and_normalized`

### MG-REQ-1034 · Key rotation is explicit

Rotating the PII key MUST be done only with `mergus secret rotate --pii`, which rebuilds the
affected raw and gold tables and is audited.

- Kind: behavior
- Verify: test `mg_req_1034_pii_rotation_rebuilds_and_audits`

### MG-REQ-1035 · Labels follow lineage

Every column derived from a `pii` column MUST inherit the label. When lineage cannot prove
independence (for example opaque Polars code), the output inherits it. A label MAY be removed
only with `pii = "none"` and a reason, which is audited.

- Kind: behavior
- Verify: test `mg_req_1035_pii_label_inherited_conservatively`

### MG-REQ-1036 · Gold gate for allowed PII

Publishing a gold column labelled `pii` with decision `allow` MUST be an error in stage 4
unless the model declares `pii = { <column> = { allow = true, reason = "..." } }`.
Pseudonymized columns pass.

- Kind: behavior
- Verify: test `mg_req_1036_gold_gate_blocks_allowed_pii`

### MG-REQ-1037 · Labels in the manifest

The manifest MUST list, per table and column, the labels `pii` and `pseudonymized`.

- Kind: behavior
- Verify: test `mg_req_1037_labels_in_manifest`
