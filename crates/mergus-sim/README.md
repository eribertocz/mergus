# mergus-sim

Deterministic simulator.

## What it does

- Gives every port of `mergus-ports` a simulated implementation (MG-REQ-0131): `SimClock`,
  `SimFileSystem`, `SimObjectStore`, `SimNetwork`, `SimSourceConnector`, `SimRandom`,
  `SimProcess` and `SimEnvironment`.
- Makes each one deterministic: created with the same seed and called the same way, it
  returns the same results.
- Injects the faults of MG-REQ-0161 that belong to a port, at a `Rate` per call:
  - object store: errors, latency, partial responses and lost writes;
  - clock: jumps and skew;
  - sources: changes, failures and partial data mid-extraction;
  - network: errors, latency and partial responses.

## What it does not

- It does not touch the outside world: time is virtual and sleeping returns at once.
- It does not choose seeds, replay or shrink runs, or accelerate time; that belongs to the
  simulation runner.
- It does not inject process crashes, concurrent writers or simultaneous triggers; they come
  in later phases.
- It is never a normal dependency of a product crate (MG-REQ-0126.8).
