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

A secret reference is `secret:<name>`, where `<name>` is a name of MG-REQ-0305. It MUST be
resolved by the first of these that answers:

1. MG-REQ-1001.1: the environment variable `MERGUS_SECRET_<NAME>`, where `<NAME>` is `<name>`
   in ASCII upper case; a variable that is set, even to an empty value, answers;
2. MG-REQ-1001.2: the operating system keychain (Keychain, Credential Manager, Secret
   Service), through the `Keychain` port, under the service `mergus` and the account `<name>`;
3. MG-REQ-1001.3: when neither answers, resolution MUST fail with a typed error that names
   the reference and both places looked at, and never a value.

Cloud secret manager plugins come after the MVP (`roadmap.md` section 4).

- Kind: behavior
- Verify: test `mg_req_1001_resolution_chain_order`
- Decision: 0031

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

### MG-REQ-1006 · Cloud credentials for object storage

Credentials of object storage that no secret reference names MUST come from federated
identity (OIDC) and the cloud's standard credential chain, never from a project file.

- Kind: behavior
- Verify: test `mg_req_1006_object_storage_uses_cloud_chain`
- Phase: 0.9
- Decision: 0031

### MG-REQ-1007 · mergus secret set

`mergus secret set <name>` MUST read the value from standard input, without echo when it is a
terminal, and store it in the keychain under the service `mergus` and the account `<name>`.
The value MUST NOT be accepted as an argument. It prints no value and exits 0 once stored:

1. MG-REQ-1007.1: the value is all of standard input with one trailing line ending (`\n` or
   `\r\n`) removed, so a value of several lines, such as a PEM key, is kept whole;
2. MG-REQ-1007.2: when standard input is a terminal, the prompt `Value of secret <name>: ` is
   written once to standard error, and the value ends at the first line ending;
3. MG-REQ-1007.3: an empty value is refused with `MG-1001` (exit code 1) and nothing is
   stored;
4. MG-REQ-1007.4: a keychain that cannot be written fails with `MG-1002` (exit code 1), whose
   fix names the platform keychain service; nothing else is stored instead;
5. MG-REQ-1007.5: a name that is not a lowercase identifier fails with `MG-1305`, and a
   second argument fails with `MG-1301`; no diagnostic ever contains the value.

- Kind: behavior
- Verify: test `mg_req_1007_secret_set_stores_in_keychain`
- Depends: MG-REQ-1001
- Decision: 0031, 0039
