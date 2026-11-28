---
id: cli-idempotency
title: Idempotency keys of state-changing commands
crate: mergus-cli
phase: "0.4"
status: draft
depends_on: [cli-commands, contracts-events]
---

# Idempotency

### MG-REQ-1313 · Idempotency keys

Every command that changes state MUST accept `--idempotency-key <key>`. A key lives 24 hours in
`.mergus/idempotency/<sha256(key)>.json` with the argument hash, exit code and output:

1. MG-REQ-1313.1: a second call with the same key and the same arguments returns the first
   call's output and exit code without new effects. The same key with other arguments fails
   with a typed `MG-13xx` error and exit code 1.
2. MG-REQ-1313.2: a call claims its key before it runs, by creating the record exclusively, and
   writes the result into it atomically when it ends. A call that finds the key claimed by a
   call still running fails with the next free `MG-13xx` code and exit code 3, which is
   retryable, and has no effect.
3. MG-REQ-1313.3: a result whose exit code is retryable (3, 4 or 7, MG-REQ-0262) is not kept.
   The claim is released, so a retry with the same key runs the command again.
4. MG-REQ-1313.4: output is written as the command runs, as it is without a key; the record
   keeps a copy of it.

- Kind: behavior
- Verify: test `mg_req_1313_same_key_has_no_second_effect`
- Phase: 0.4
- Decision: 0010
