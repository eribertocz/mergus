# 0014 · What Mimus needs from Mergus

- Status: accepted
- Date: 2026-11-21
- Answers: request of the Mimus specifier, approved by the owner

## Context

Mimus is the optional AI module of the ecosystem. Its goal is that an agent does everything a
person does with Mergus, with the same security, while Mergus holds no AI (MG-REQ-0190,
MG-REQ-0192). Its specifier listed the contracts it needs. Most exist; some gaps remained.

## Decision

- `describe` also gives lineage identifiers, tests, health, last run and PII labels, without
  values (MG-REQ-1314, 0.7).
- `mergus profile` gives row counts, column profiles and duplicate key counts, without minimum
  and maximum on PII columns (MG-REQ-1315, 0.7).
- `diff` accepts a branch environment and counts added, removed and changed rows by key
  (MG-REQ-1228, 0.7).
- `mergus run status --wait --timeout` lets an agent wait for a run without polling
  (MG-REQ-1316, 0.7).
- `actor` is an object with `id` and `on_behalf_of` (MG-REQ-0264, 0.2).
- Owner decisions: each module publishes its own thin MCP server; Mergus's is `mergus-mcp`, in
  its own repository and versioned with the CLI (MG-REQ-0290). The "Shared AI module" consumer
  is Mimus, which uses the contracts directly, not MCP.
- Git stays outside Mergus (MG-REQ-0191): branches, commits and pull requests belong to Mimus or
  the host. `plan --json` and `diff --json` are the machine-readable summary of a change.

## Consequences

The new rules join their phases; no current issue changes.
