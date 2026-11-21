# 0010 · Agents are first-class users

- Status: accepted
- Date: 2026-11-21
- Builds on: decision 0008

## Context

Decision 0008 lets any agent use Mergus with the same doors as a person. The owner asked for
agents to be first-class users before the MVP, still with no AI inside Mergus.

## Decision

| Change | Rule | Phase |
|---|---|---|
| The MCP adapter moves from 1.4 to 0.8 | MG-REQ-0290 | 0.8 |
| Every diagnostic carries a `hint`, and optionally `next` CLI invocations | MG-REQ-1334 | 0.2 |
| An agent guide, generated and linked from `llms.txt` | MG-REQ-1787 | 0.8 |

## Consequences

Agents get machine-readable next steps from 0.2 on and an MCP adapter before 1.0. Nothing is
executed automatically, and approval stays with Git (MG-REQ-0191).
