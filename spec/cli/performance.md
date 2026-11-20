---
id: cli-performance
title: Performance targets
crate: mergus-cli
phase: "0.9"
status: draft
depends_on: [architecture-principles]
---

# Performance targets

Every target is a benchmark in CI (MG-REQ-0181), on the reference machines described in
`bench/README.md`.

### MG-REQ-1341 · CLI responsiveness

`mergus --help` MUST start in under 50 ms; `validate` of a 200-model project without querying
sources MUST finish in under 1 s; completions MUST answer in under 100 ms.

- Kind: behavior
- Verify: test `mg_req_1341_cli_latency_targets`

### MG-REQ-1342 · Data volume targets

On a 16 GB laptop Mergus MUST complete the TPC-H SF10 reference project; on the reference
server SF100; and a model five times larger than RAM MUST finish using spill.

- Kind: behavior
- Verify: test `mg_req_1342_volume_targets`

### MG-REQ-1343 · Binary size and startup

The core binary MUST be under 40 MB and start in under 20 ms.

- Kind: behavior
- Verify: test `mg_req_1343_binary_size_and_startup`
