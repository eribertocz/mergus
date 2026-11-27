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

- Records every load in `ops.loads` (MG-REQ-0514) with `load_raw`, after the raw commit,
  through the adapter of the ops zone. A failed load, one refused with `MG-0504` too, is
  recorded with `result = 'failed'`, 0 rows, an unchanged watermark and its code.
  `load_raw_off` records the extraction of a source with `raw = false` as `raw_off` and writes
  nothing to raw (MG-REQ-0517). `new_load_id` makes the load id, a UUID version 7, and
  `last_watermark` gives the last recorded `watermark_after`, where the next load starts.
- Gives the retention of each source, 30 days unless it sets `retention`, and expires the
  shared raw catalog with the longest of the sources that keep raw (MG-REQ-0516), through the
  adapter's `expire`.

- Loads an incremental table by chunks with `load_table` (MG-REQ-0412, MG-REQ-0413,
  MG-REQ-0415, MG-REQ-0418, MG-REQ-0420): each chunk is a commit that appends to
  `__changes` the rows that differ from the current row of their key and the rows the delete
  column marks deleted; the last commit appends the deletes of a due `compare` and rewrites
  the current table; only then is the `ops.loads` row written, with the largest incremental
  value of the committed rows as `watermark_after`, the rows read as `rows` and, after a
  compare, its time as `deletes_checked_at`, NULL otherwise. A key read twice across the
  chunks is `MG-0504`, recorded `failed`; readers see only loads recorded `ok`. A load id of `__changes` with no `ops.loads` row is resumed with the
  same id after its largest key, or only recorded when its last commit was made; without
  `key` it is ignored and the table read again.
  `finished_changes` gives the rows of `__changes` readers see.

## What it does not

- It does not check the keys of a zone: the schema of `mergus-project` rejects an unknown
  key or profile, a missing `profile` or `location`, and a `cloud` zone without a `catalog`
  secret reference.
- It reads no secret: a `cloud` catalog's connection value is given resolved. The caller
  gives the version of the table's last commit and the watermarks of each load.
- It does not expire raw during a run: `expire_raw` is maintenance work (MG-REQ-0513).
- `write_raw` does not detect deletes: its load carries the keys found deleted. `load_table`
  has `mergus-source` detect them.
- The current table is rewritten in full at each load.
