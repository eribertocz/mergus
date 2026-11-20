---
id: architecture-principles
title: Product guarantees above any feature
crate: none
phase: "0.1"
status: draft
depends_on: [kernel]
---

# Principles

### MG-REQ-0180 · Correctness over completion

Mergus MUST NOT produce a wrong result silently. Decimals keep exact precision, NULL keeps
SQL semantics, timestamps keep their time zone. A value that cannot be represented exactly in
the target type MUST fail the step with a typed error instead of being rounded, truncated or
cast.

- Kind: behavior
- Verify: test `mg_req_0180_unrepresentable_value_fails`
- Phase: 0.4

### MG-REQ-0181 · Efficiency is measured

Every performance claim MUST be backed by a benchmark that runs in CI. A pull request that
regresses a benchmark beyond its threshold MUST fail.

- Kind: process
- Verify: check `benchmarks`

### MG-REQ-0182 · AI is optional and goes through the same doors

Every feature MUST work without AI. Any action proposed by an AI agent MUST use the same CLI
commands, plans, `--dry-run` and permissions as a human, with no private path.

- Kind: behavior
- Verify: review `ai-same-doors`

### MG-REQ-0183 · Models have an owner

A model MAY declare `owner` in its folder `mergus.toml`. When declared, incidents and the
trust page MUST show it.

- Kind: behavior
- Verify: test `mg_req_0183_owner_shown_on_incident`
- Phase: 0.7
