---
id: security-secrets
title: Secret references, resolution and handling in memory
crate: mergus-security
phase: "0.3"
status: draft
depends_on: [kernel, architecture-boundaries]
---

# Secrets

### MG-REQ-1001 · Resolution chain

A reference `secret:<name>` MUST be resolved by the first of these that answers:
1. the environment variable `MERGUS_SECRET_<NAME>`;
2. the operating system keychain (Keychain, Credential Manager, Secret Service), set with
   `mergus secret set <name>`;
3. a cloud secret manager plugin (after the MVP);
4. federated identity (OIDC) and the cloud's standard credential chain, for object storage.

- Kind: behavior
- Verify: test `mg_req_1001_resolution_chain_order`

### MG-REQ-1002 · Secrets cannot be printed

Secret values MUST be held in a type that cannot be printed, formatted, serialized or logged
(`secrecy`). Connection URLs MUST always be shown without password.

- Kind: behavior
- Verify: test `mg_req_1002_secret_type_never_displays`

### MG-REQ-1003 · Clean environment for Python

Before loading user Python code, the interpreter's process MUST start with an environment
from which every `MERGUS_SECRET_*` variable and every credential variable of known cloud SDKs
has been removed. Python steps receive only Arrow batches.

- Kind: behavior
- Verify: test `mg_req_1003_python_env_has_no_credentials`
- Phase: 0.4
- Depends: MG-REQ-0006

### MG-REQ-1004 · Doctor shows origins, never values

`mergus doctor` MUST show, for each secret reference, which link of the chain resolved it, and
never its value.

- Kind: behavior
- Verify: test `mg_req_1004_doctor_shows_secret_origin`

### MG-REQ-1005 · No .env files

Mergus MUST NOT read `.env` files.

- Kind: behavior
- Verify: test `mg_req_1005_dotenv_ignored`
