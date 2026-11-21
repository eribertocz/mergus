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

### MG-REQ-0190 · No AI inside Mergus

Mergus MUST NOT embed or call a language model, an AI SDK or an AI provider. Explanations
(MG-REQ-1425) and suggestions (MG-REQ-0353) are deterministic cases of this rule. AI lives in
separate modules that use Mergus like a person does (MG-REQ-0182).

- Kind: behavior
- Verify: review `no-ai-inside`
- Decision: 0008

### MG-REQ-0191 · Approval by Git

A change proposed by an agent MUST reach a project only as a commit on a branch that a person
reviews (`validate`, `plan`, `explain`) and merges; production is promoted only from `main`
(MG-REQ-1225). The runtime has no separate approval step and no path that skips the branch.

- Kind: process
- Verify: review `approval-by-git`
- Depends: MG-REQ-0182, MG-REQ-1225
- Decision: 0008
