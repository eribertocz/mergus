---
id: architecture-testing
title: Test levels, simulator, differential tests and traceability
crate: mergus-sim
phase: "0.1"
status: draft
depends_on: [architecture-invariants]
---

# Testing

## 1. Levels

| Level | Covers | Tool | Runs |
|---|---|---|---|
| Unit and property | Planner, fingerprints, tokens, hashes, parsers | proptest | Every PR |
| Snapshot | Plans, errors, catalog, manifests, CLI output and events | insta | Every PR |
| SQL logic | Transformations end to end through the public interface | sqllogictest-rs | Every PR |
| Contract | Every emitted JSON against its schema and against Menura | Menura CLI | Every PR |
| Differential | The pairs of section 3 | proptest + testkit | PR sample, nightly full |
| Mutation | That tests really check | cargo-mutants | PR on diff, nightly full |
| Simulation | Invariants under faults | mergus-sim | PR 1,000 seeds, continuous |
| End to end | Real binary, PostgreSQL, S3-compatible storage, DuckLake catalogs | nextest + containers | Nightly, release |
| Python | `mergus` package and Polars steps | pytest | Every PR |
| Fuzzing | TOML files, selectors, JSON input | cargo-fuzz | Nightly |
| Performance | Benchmarks and the targets of `cli/performance.md` | divan + CodSpeed | Every PR |

Runner: `cargo nextest`.

### MG-REQ-0160 · Flaky tests are quarantined

A test that fails intermittently MUST be quarantined the same day with an issue. Silent
retries MUST NOT be used.

- Kind: process
- Verify: review `flaky-quarantine`

## 2. Simulator

`mergus-sim` runs the real runtime with simulated ports, in one process, driven by a seed.

### MG-REQ-0161 · Faults the simulator injects

The simulator MUST be able to inject: process crash at any step, including between two
writes; object-store errors, latency, partial responses and lost writes; DuckLake catalog
errors; sources that change, fail or return partial data mid-extraction; clock jumps and
skew; concurrent writers and readers; simultaneous triggers.

- Kind: behavior
- Verify: test `mg_req_0161_simulator_injects_every_fault`
- Phase: 0.5

### MG-REQ-0162 · Exact replay and shrinking

`mergus-sim --seed <n> --commit <sha>` MUST replay a failure step by step, and the simulator
MUST shrink a failing case to a minimal one that still fails.

- Kind: behavior
- Verify: test `mg_req_0162_seed_replays_and_shrinks`

### MG-REQ-0163 · Accelerated time

Simulated time MUST advance without waiting, so that hours of schedules, retries and
retention run in seconds.

- Kind: behavior
- Verify: test `mg_req_0163_simulated_time_is_accelerated`

## 3. Differential tests

| Path under test | Reference |
|---|---|
| Incremental load by periods | Full rebuild |
| Step in DuckDB | Same step in Polars, when expressible in both |
| Fused segments and column pruning | Steps executed one by one |
| Pinned read | Direct read of the version |

### MG-REQ-0164 · Differential equivalence

Each pair MUST produce identical results, or equivalent under the documented rules (for
example row order without `ORDER BY`).

- Kind: behavior
- Verify: test `mg_req_0164_differential_pairs_agree`
- Phase: 0.5

## 4. Traceability

### MG-REQ-0165 · Traceability report

`cargo xtask traceability` MUST list behavior rules without a test, tests whose
`mg_req_<id>` prefix names no rule, and rules whose test fails. CI MUST block a pull request
that leaves a behavior rule of its issue without a passing test.

- Kind: process
- Verify: check `traceability`

### MG-REQ-0167 · Scope of the traceability report

For MG-REQ-0165, a rule "needs a test" when it is a behavior rule whose `Verify` is `test`;
rules verified by a check or a review are not listed as missing a test. The current phase is
read from the line `Current phase: <major>.<minor>` of `spec/roadmap.md`, which the specifier
advances when it files the issues of the next phase; rules of a later phase are not listed as missing a test.

- Kind: process
- Verify: check `traceability`
- Depends: MG-REQ-0165

## 5. Continuous integration budget

### MG-REQ-0166 · Pull request CI under ten minutes

The blocking checks of a pull request MUST finish in under ten minutes on Linux, macOS and
Windows when the build cache is warm. A run on a cold cache (after a `Cargo.lock` change that
adds native code, such as bundled DuckDB) MAY take up to thirty minutes, sixty on Windows,
which are the jobs' time limits. `cargo xtask ci` MUST run locally exactly what CI runs.

- Kind: process
- Verify: check `ci-duration`
- Phase: 0.1
- Decision: 0029, 0030

### MG-REQ-0168 · External agent compatibility

1. MG-REQ-0168.1: a nightly, non-blocking job MUST drive Mergus as an external agent would,
   through the CLI with `--json` only: `describe`, a change on a branch, `validate`, `plan`,
   and a denied permission that ends with its exit code. The agent is a deterministic stub, so
   the job needs no AI provider.
2. MG-REQ-0168.2: the 1.0 release notes MUST include a compatibility table of the agents
   tested against Mergus.

- Kind: process
- Verify: check `agent-compatibility`
- Phase: 0.9
- Depends: MG-REQ-0182, MG-REQ-1312
- Decision: 0008

### MG-REQ-0169 · Agent benchmark

A fixed, versioned set of agent tasks with checkable outcomes MUST run against the agents
listed in MG-REQ-0168.2, and the 1.0 release notes MUST publish each agent's success rate. The
job never blocks a release.

- Kind: process
- Verify: check `agent-benchmark`
- Phase: 0.9
- Depends: MG-REQ-0168
- Decision: 0011
