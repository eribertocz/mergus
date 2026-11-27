# mergus-sim architecture

## Main types

| Type | Role |
|---|---|
| `Rate` | A fault probability in parts per million, from `Rate::NEVER` to `Rate::ALWAYS` |
| `SimClock`, `ClockFaults` | Virtual wall and monotonic time; jumps and skew |
| `SimFileSystem` | Files and directories in memory, keyed by path components; `open` reads a cursor over the stored bytes, `create` stores a stream on flush and on drop |
| `SimObjectStore`, `ObjectStoreFaults` | Objects in memory; errors, latency, partial, lost writes |
| `SimNetwork`, `NetworkFaults`, `TcpHandler` | HTTP routes and TCP peers set by the test |
| `SimSourceConnector`, `SourceFaults` | Sources of batches, with a changed version |
| `SimRandom` | Seeded random numbers |
| `SimProcess` | Programs answer with outputs set by the test; `runs` returns every command run |
| `SimEnvironment` | Arguments, current directory, variables, a configuration and a cache directory set by the test |
| `SimDatabase`, `DatabaseFaults` | Results and extension versions recorded with the real `Database`, replayed batch by batch by `query_arrow_each`; the statements run, appended batches and attached catalogs, encrypted or not; catalogs recorded unencrypted; errors |
| `SimPython` | Python trees recorded with the real `Python` port, by source text, read with `PythonTree::from_json` |
| `SimKeychain` | Keychain entries in memory, by service and account, set by the test |
| `Scenario` | What a caller simulates: how a seed draws steps, the world, one step |
| `Simulator` | Runs, replays and shrinks a scenario from a seed |
| `Replay`, `StepRecord` | A run replayed one step at a time, and each step's result |
| `Outcome`, `Failure`, `Violation` | A run's result; a failing case and what it broke |

## Data flow

1. A test creates each simulated port with a seed and its faults. The object store and the
   network share a `SimClock`: injected latency advances it.
2. It passes the ports, as trait objects, to the code under test.
3. Each call draws its faults from a private SplitMix64 generator, behind a `Mutex`. A call
   draws the same numbers whatever the rates, so the sequence depends only on the seed and
   the calls.
4. State lives in `BTreeMap`s, so listings are sorted and never depend on hashing.
5. `Simulator::run(seed)` draws the steps from a generator derived from the seed, builds a
   fresh world with `Scenario::start(seed)` and applies the steps until one breaks an
   invariant. The same seed gives the same steps and the same world, so `replay` repeats a
   failure step by step.
6. `Simulator::shrink` removes runs of steps, halving their length down to one, and keeps
   each removal after which the case still fails, until no single step can be removed.
7. Scenarios live with the code they drive, as tests: INV-04 in the tests of
   `mergus-runtime`. `cargo xtask simulation` gives them their seeds through the environment
   and runs them (MG-REQ-0198).

## Dependencies

- `mergus-ports`, without the feature `real`: the traits it implements.
- `arrow` (default features off): the record batches of the simulated `Database`.
- The standard library only for collections and locks; no I/O.
