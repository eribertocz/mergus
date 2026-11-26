# mergus-certify architecture

## Main types

| Type | Role |
|---|---|
| `Harness` | What an adapter gives the kit: `store` makes a new store, `open` a client of it |
| `Faults` | The crash plan of one client; the harness asks it before every storage call |
| `certify` | Runs every check on a harness and returns the `Report` |
| `Report`, `Outcome`, `Status` | Every check and its result: passed, not applicable, failed |

## Data flow

1. `certify` runs each check in turn; each check asks the harness for stores of its own.
2. A check opens clients, writes one table `mergus_certify` with one `BIGINT` column `id`,
   commits and reads it back with `read_at`, and compares the rows with what the write
   modes say.
3. The crash check arms `Faults` to crash the client after 0, 1, 2… storage calls from the
   start of a write, drops the crashed client, and opens a new one to inspect the store. It
   stops at the first point where the write and its commit complete.
4. Every failure becomes `Status::Failed` with its reason; no check panics.
5. The kit calls `pin_recipe` in one place only, `calls::pin`.

## Dependencies

- `mergus-adapter`: the trait and its types (shared layer-3 crate, MG-REQ-0120).
- `arrow`, without default features: the record batches the kit writes and reads.
- Dev-dependencies: `mergus-adapter-ducklake`, `mergus-ports` with `real` and `mergus-sim`,
  for the DuckLake run and its crash faults.
