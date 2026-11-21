# 0011 · Agents best in class

- Status: accepted
- Date: 2026-11-21
- Builds on: decisions 0008 and 0010

## Context

The owner asked for agent support of the first level, better than the leading tools. Their
practice (dbt, Supabase, Dagster, Airflow, Fivetran) and the MCP specification show what
leaders do: read-only and scoped modes, tool groups, structured outputs, human confirmation
through the client, packaged agent skills, and awareness of prompt injection from data.

## Decision

| Rule | What | Phase |
|---|---|---|
| MG-REQ-0291 | MCP read-only and dev-only by default; tool groups `discover`, `build`, `operate` | 0.8 |
| MG-REQ-0292 | Values read from data are marked as untrusted data, never instructions | 0.8 |
| MG-REQ-0293 | Irreversible actions ask the person through the MCP client; no automatic yes | 0.8 |
| MG-REQ-0294 | Every MCP tool declares an output schema from Mergus contracts | 0.8 |
| MG-REQ-1788 | Versioned agent skills for common jobs, each with a tested example | 0.8 |
| MG-REQ-0169 | Agent benchmark with published success rates in 1.0 | 0.9 |

## Consequences

Together with MG-REQ-0192 (everything a person can do, an agent can do), Mergus offers a safe
default, proof of quality and recipes, with no AI inside (MG-REQ-0190).
