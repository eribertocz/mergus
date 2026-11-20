# mergus-sim architecture

## Main types

| Type | Role |
|---|---|
| `Rate` | A fault probability in parts per million, from `Rate::NEVER` to `Rate::ALWAYS` |
| `SimClock`, `ClockFaults` | Virtual wall and monotonic time; jumps and skew |
| `SimFileSystem` | Files and directories in memory, keyed by path components |
| `SimObjectStore`, `ObjectStoreFaults` | Objects in memory; errors, latency, partial, lost writes |
| `SimNetwork`, `NetworkFaults`, `TcpHandler` | HTTP routes and TCP peers set by the test |
| `SimSourceConnector`, `SourceFaults` | Sources of batches, with a changed version |
| `SimRandom` | Seeded random numbers |
| `SimProcess` | Programs answer with outputs set by the test |
| `SimEnvironment` | Variables and a configuration directory set by the test |

## Data flow

1. A test creates each simulated port with a seed and its faults. The object store and the
   network share a `SimClock`: injected latency advances it.
2. It passes the ports, as trait objects, to the code under test.
3. Each call draws its faults from a private SplitMix64 generator, behind a `Mutex`. A call
   draws the same numbers whatever the rates, so the sequence depends only on the seed and
   the calls.
4. State lives in `BTreeMap`s, so listings are sorted and never depend on hashing.

## Dependencies

- `mergus-ports`, without the feature `real`: the traits it implements.
- The standard library only for collections and locks; no I/O.
