# mergus-certify

Conformance kit every storage adapter must pass.

## What it does

- Runs one suite of checks on any `StorageAdapter` through a `Harness` the adapter provides:
  fresh stores, and clients opened on a store (MG-REQ-0289).
- Covers the six operations: a stable declaration, each declared write mode, rows invisible
  before commit (MG-REQ-0286), new readable versions on every commit, `pin_recipe` of known
  and unknown versions, `VersionUnavailable` for an unknown version (MG-REQ-0252), and
  `expire` keeping live versions and the rollback window (MG-REQ-0288).
- Crashes a client at every storage call from the start of a write, through the time between
  write and commit, to the end of the commit. After each crash a new client finds the
  published version intact and the crashed write published whole or not at all.
- Checks concurrency: a client moved between threads, a later client seeing earlier commits,
  and, when concurrent clients are declared, two clients appending at once without losing
  rows.
- Returns a `Report` with one `Outcome` per check: passed, failed with the reason, or not
  applicable when the adapter's declaration rules the check out.
- Runs on the DuckLake adapter in its own tests, over a real DuckLake catalog.

## What it does not

- It does not drive the replace-period and merge-by-key write modes: the contract does not
  carry their period or key yet. They are reported not applicable.
- It does not crash real processes: the harness fails the client's storage calls when
  `Faults::crashed` says so. The DuckLake test routes them to the simulator's database
  faults.
- It does not check multi-table transactions or the exact storage of each declared type.
