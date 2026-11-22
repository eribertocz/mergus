# 0026 · The plan in phase 0.2

- Status: accepted
- Date: 2026-11-22
- Answers: open questions of #83

## Context

The plan of `run/plan.md` names results, input versions and a resource budget that do not
exist before storage (0.3), execution (0.4) and the resource governor (0.9).

## Decision

- The plan is the contract `mergus_plan`; `plan_id` and fingerprints are SHA-256 of canonical
  JSON, with CRLF normalized so Windows checkouts give the same plan (MG-REQ-0743).
- Results, input versions and engine versions are inputs given by the caller; in 0.2 there are
  none, so every step computes and raw inputs are `null`.
- The budget is `max_parallel`, recorded in the plan, so the plan does not depend on the
  machine. Groups follow depth, then name.
- The plan file lives under `.mergus/plans/`; the `plan` command joins the CLI issues.

## Consequences

When storage arrives, the caller fills results and versions; the contract does not change.
