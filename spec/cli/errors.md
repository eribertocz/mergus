---
id: cli-errors
title: Error code catalog
crate: mergus-cli
phase: "0.2"
status: draft
depends_on: [cli-output]
---

# Error codes

Error codes are `MG-` plus four digits. The first two digits name the area, mirroring the rule
ranges of `spec/README.md` (for example `MG-04xx` extraction, `MG-10xx` security). The
catalog lives in `crates/mergus-cli/errors/MG-####.md`, one file per code.

| Range | Area | Range | Area |
|---|---|---|---|
| MG-01xx | architecture, internal | MG-09xx | publish |
| MG-02xx | contracts | MG-10xx | security |
| MG-03xx | project | MG-11xx | observability |
| MG-04xx | extract | MG-12xx | environments |
| MG-05xx | storage | MG-13xx | cli |
| MG-06xx | model | MG-15xx | ops |
| MG-07xx | run | MG-16xx | distribution |
| MG-08xx | quality | MG-14xx | ui |
| | | MG-17xx | governance |

### MG-REQ-1331 · Every code is documented

Each code MUST have a file with: title, exit code, retryable or not, cause, example, fix, and
the rule IDs it enforces. CI MUST fail when code emits an undocumented code or a documented
code is never emitted by any test.

- Kind: process
- Verify: check `error-catalog`

### MG-REQ-1332 · Codes are stable

An error code MUST never change meaning or be reused. A retired code keeps its file with a
`retired` note.

- Kind: process
- Verify: check `error-catalog`

### MG-REQ-1333 · Code range matches area

A code MUST be in the range of the area of the rule it enforces.

- Kind: process
- Verify: check `error-catalog`
