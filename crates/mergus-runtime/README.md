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

- Opens raw with `open_raw`: the DuckLake catalog is attached as `raw` with the `ENCRYPTED`
  option, with data inlining off, so every row lands in an encrypted Parquet file
  (MG-REQ-0515.1). A `local` or `shared` catalog inside the project is refused with `MG-0502`
  before anything is attached (MG-REQ-0515.2), and an existing unencrypted catalog with
  `MG-0503` (MG-REQ-0515.3).
- Writes one load of a declared table with `write_raw`, in one commit (MG-REQ-0518.1):

  | Table | Holds | Write |
  |---|---|---|
  | `raw.<source>__<table>__changes` | every version, and each detected delete with the last known values | `append` |
  | `raw.<source>__<table>` | the current, not deleted row per key; without `key`, the rows of the latest load | `replace` |

  Both keep the source's columns with their native types (MG-REQ-0511), followed by
  `_load_id VARCHAR`, `_extracted_at TIMESTAMPTZ`, `_deleted BOOLEAN` and
  `_deleted_at TIMESTAMPTZ` (MG-REQ-0512). A load that returns a key twice fails with
  `MG-0504` and writes nothing (MG-REQ-0518.3).

## What it does not

- It does not check the keys of a zone: the schema of `mergus-project` rejects an unknown
  key or profile, a missing `profile` or `location`, and a `cloud` zone without a `catalog`
  secret reference.
- It reads no secret: a `cloud` catalog's connection value is given resolved. It keeps no
  load registry and expires nothing (#195): the caller gives each load's id and the version
  of the table's last commit.
- It does not detect deletes: a load carries the keys found deleted.
- The current table is rewritten in full at each load.
