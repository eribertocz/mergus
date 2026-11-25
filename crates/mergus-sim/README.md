# mergus-sim

Deterministic simulator.

## What it does

- Gives every port of `mergus-ports` a simulated implementation (MG-REQ-0131): `SimClock`,
  `SimFileSystem`, `SimObjectStore`, `SimNetwork`, `SimSourceConnector`, `SimRandom`,
  `SimProcess`, `SimEnvironment`, `SimKeychain` and `SimDatabase`, which answers from results
  recorded with the real `Database` (MG-REQ-0194).
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
- It does not run the real runtime yet: the caller gives the scenario. Running the runtime,
  and a `mergus-sim` command, come with later phases.
- It does not inject process crashes, concurrent writers or simultaneous triggers; they come
  in later phases.
- It is never a normal dependency of a product crate (MG-REQ-0126.8).
