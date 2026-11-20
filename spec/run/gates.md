---
id: run-gates
title: Gates checked before any data moves
crate: mergus-plan
phase: "0.4"
status: draft
depends_on: [run-plan]
---

# Gates

All gates run in stage 4. If any blocks, no data moves and each error names the gate, the
model, the step and the fix.

| Gate | Blocks when | Rule |
|---|---|---|
| Type passport | A boundary loses information | MG-REQ-0634 |
| SQL safety | A step reads something undeclared, writes, attaches or uses the network | MG-REQ-0308 |
| Determinism | A published model uses undeclared non-deterministic operations | MG-REQ-0637 |
| Output contract | The computed schema differs from `contracts/<model>.json` | MG-REQ-0802 |
| Schema gate | Menura `validate` reports errors or new `W0*` warnings | MG-REQ-0232 |
| Storage rules | A guarantee rule of `storage/profiles.md` is violated | MG-REQ-0523 |
| Credentials | A required secret is missing (checked without reading its value) | MG-REQ-0721 |
| PII | A column labelled `pii` has no decision | MG-REQ-0146 |

### MG-REQ-0720 · Every gate runs before execution

Mergus MUST evaluate every gate of this table in stage 4 and MUST NOT start stage 5 if any
gate blocks.

- Kind: behavior
- Verify: test `mg_req_0720_blocked_gate_prevents_execution`

### MG-REQ-0721 · Credentials checked without reading them

The credentials gate MUST confirm that every required secret reference resolves, without
reading or printing its value.

- Kind: behavior
- Verify: test `mg_req_0721_secret_existence_checked_only`

### MG-REQ-0722 · All gate errors together

Stage 4 MUST report the errors of every gate in one output, not only the first gate that
blocks.

- Kind: behavior
- Verify: test `mg_req_0722_all_gate_errors_reported`
