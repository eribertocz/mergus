# 0050 · Invariants under the simulator: 1,000 seeds per pull request, 24 hours before each release

- Status: accepted
- Date: 2026-11-27
- Answers: the Invariants gate of release 0.3 (issue #237). INV-04 (MG-REQ-0143) is the first
  invariant due, and nothing ran it under seeds.

## Context

`spec/roadmap.md` §2 asks for 1,000 simulator seeds per pull request and "24 hours of
continuous simulation". Nothing in CI ran either. Simulation that runs without pause costs
about 43,000 Linux minutes a month on hosted runners.

## Decision

- **Every pull request (MG-REQ-0198.1):** it runs 1,000 seeds of every invariant that is due,
  within the ten-minute CI budget, which simulated time makes possible.
- **Before every release (MG-REQ-0198.2):** a 24-hour campaign runs on `main`, as four chained
  jobs of six hours each with new seeds. It costs about 1,440 minutes per release and is the
  evidence of the gate.
- **The gate's wording:** it now reads "a 24-hour simulation campaign on `main` before the
  release". The 24 hours remain; only when they run changes.

## Consequences

- A release waits at least 24 hours after its last merge to `main`.
- A failing seed always comes with its replay command.
