# 0008 · AI-ready, with no AI inside

- Status: accepted
- Date: 2026-11-21
- Aligned with: Menura decision 0004 and Menura issue #303

## Context

The ecosystem wants any agent to use Mergus with the same permissions and controls as a
person, without AI inside Mergus and without depending on a provider. The specification
already had MG-REQ-0182 (same doors), MG-REQ-1425 and MG-REQ-0353 (no AI in explanations and
suggestions), MG-REQ-0260 to MG-REQ-0263 (CLI `--json`, exit codes, `schema`) and MG-REQ-1225
(prod only from `main`). It lacked a general rule, a trusted actor, a filtered description of
the project, a way to mount ecosystem pieces, and a place for the AI module and MCP adapter.

## Decision

| # | Decision | Rule | Gap it closes | Phase |
|---|---|---|---|---|
| 1 | No LLM, AI SDK or AI provider inside Mergus | MG-REQ-0190 | Only per-feature rules existed | 0.1 |
| 2 | Approval by Git: branch, review, merge; prod only from `main`; no runtime approval step | MG-REQ-0191 | No explicit approval for agent changes | 0.1 |
| 3 | `actor` in events and audit, set only by host or authenticated CLI, audit only | MG-REQ-0264, MG-REQ-1107 | Audit did not record who proposed or ran | 0.2, 0.7 |
| 4 | `describe --json`, filtered by identity; hidden equals absent; no data or secrets | MG-REQ-1312 | No stable, filtered description of the project | 0.6 |
| 5 | Trusted ecosystem pieces in the mini-host, only if declared with integrity | MG-REQ-1407 | Conflict with MG-REQ-1406 | 1.1 |
| 6 | MCP adapter: separate, thin over CLI `--json`, same permissions | MG-REQ-0290 | Only a roadmap line | 1.4 |
| 7 | Shared AI module and MCP adapter listed as consumers | `contracts/consumers.md` | Consumers not named | 0.5 |
| 8 | Nightly non-blocking agent test with a deterministic stub; compatibility table in 1.0 | MG-REQ-0168 | No proof of compatibility | 0.9 |

Each rule enters in the phase whose area it belongs to, so the current roadmap is not delayed.

## Consequences

- MG-REQ-0182, MG-REQ-1425, MG-REQ-0353 and MG-REQ-1225 are unchanged; the new rules cite them.
- The AI module is not part of Mergus; it is a consumer bound by the same contracts as any client.
