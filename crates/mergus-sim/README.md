# mergus-sim

Deterministic simulator.

## What it does

- Gives every port of `mergus-ports` a simulated implementation (MG-REQ-0131): `SimClock`,
  `SimFileSystem`, `SimObjectStore`, `SimNetwork`, `SimSourceConnector`, `SimRandom`,
  `SimProcess`, `SimEnvironment`, `SimKeychain` and `SimDatabase`, which answers from results
  recorded with the real `Database` and logs the statements run (MG-REQ-0194, MG-REQ-0196).
- Makes each one deterministic: created with the same seed and called the same way, it
  returns the same results.
- Injects the faults of MG-REQ-0161 that belong to a port, at a `Rate` per call:
  - object store: errors, latency, partial responses and lost writes;
  - clock: jumps and skew;
  - sources: changes, failures and partial data mid-extraction;
  - network: errors, latency and partial responses;
  - database: errors.
- Runs a `Scenario` given by the caller from a seed with `Simulator`: the seed draws the steps
  and seeds the world, so a seed replays a failure exactly, step by step (MG-REQ-0162).
- Shrinks a failing run to a minimal case that still fails on the same seed (MG-REQ-0162).
- Advances simulated time without waiting, so hours of schedules and retries run in
  milliseconds (MG-REQ-0163).
- Never retries a failing run: a failure is reported with its seed and its steps, so it can
  be replayed and quarantined (MG-REQ-0160).

## What it does not

- It does not touch the outside world: time is virtual and sleeping returns at once.
- It holds no scenario: the caller gives it. The scenario of INV-04 (MG-REQ-0143) is a test
  of `mergus-runtime`, in `crates/mergus-runtime/tests/incremental/simulation.rs`, that runs
  the real runtime on these ports. `cargo xtask simulation` runs it over 1,000 seeds on every
  pull request and in the release campaign (MG-REQ-0198); a `mergus-sim` command comes with
  later phases.
- Its ports do not inject process crashes: the INV-04 scenario crashes a load itself, after
  the commit its seed chooses, through the crashing stores of the runtime's tests. Concurrent
  writers and simultaneous triggers come in later phases.
- It is never a normal dependency of a product crate (MG-REQ-0126.8).
