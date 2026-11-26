# mergus-runtime

Run cycle and scheduling; wires ports to crates.

## What it does

- Turns the storage of each zone of `project.toml` (`[raw]`, `[gold]`, `[ops]`), as
  `mergus-project` has validated it, into what DuckLake needs, from `location` alone
  (MG-REQ-0521):

  | `profile` | Catalog | Data files | Clients |
  |---|---|---|---|
  | `local` | `ducklake:<location>/catalog.ducklake` | `<location>/data/` | one process |
  | `shared` | `ducklake:sqlite:<location>/catalog.sqlite` | `<location>/data/` | several local processes |
  | `cloud` | `ducklake:postgres:<value of the secret catalog>` | `<location>` | several, remote |

  `catalog` and `data_path` override the defaults (MG-REQ-0521.2). The catalog names the one
  extension it needs besides `ducklake`: `sqlite` or `postgres` (MG-REQ-0521.3).
- Checks that raw and gold are separate stores, and reports `MG-0501` with the shared value
  when they are not (MG-REQ-0501).

## What it does not

- It does not check the keys of a zone: the schema of `mergus-project` rejects an unknown
  key or profile, a missing `profile` or `location`, and a `cloud` zone without a `catalog`
  secret reference.
- It attaches no catalog, reads no secret and writes nothing; it does not run anything yet.
