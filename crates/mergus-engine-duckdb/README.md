# mergus-engine-duckdb

Execute DuckDB steps.

## What it does

- Runs one segment (`Segment`) in its own in-memory DuckDB session, reached only through the
  `Database` port (MG-REQ-0194): the step's SQL, its bound parameters (MG-REQ-0007), the
  project's macro texts, its input tables and its output table (`Table`), each with its
  declared schema.
- Executes every macro text in each session before anything else, so every session that runs
  a segment has the project's macros (MG-REQ-0643).
- Applies the transport the plan chose for each boundary (MG-REQ-0633, MG-REQ-0704):
  - `SharedMemory`: an Arrow IPC file in the run's work directory, written and read one
    record batch at a time through the streaming `FileSystem::create` and `FileSystem::open`
    of the port, each batch appended to DuckDB before the next is decoded;
  - `Parquet`: a Parquet file in the work zone, read lazily through a view over
    `read_parquet` of the declared columns. It is written by DuckDB's `COPY` from a table of
    a second session backed by `<output file>.duckdb` next to it, so the rows sit on disk,
    not in memory; that file, its `.wal` and its `.tmp` spill directory are removed before
    and after the segment, and one that cannot be removed fails the segment.
  The step's result is streamed out of DuckDB one batch at a time
  (`DatabaseConnection::query_arrow_each`) to either transport, so Rust never holds more
  than one batch of it.
  `Transport::for_size` gives the rule: shared memory up to `MEMORY_BUDGET`, 1 GiB of Arrow
  per boundary, and Parquet above it.
- Passes every table across a boundary in the types its schema declares, or fails the step
  with `MG-0101` (MG-REQ-0180): every declared schema is checked before the session opens
  (types DuckDB holds exactly, no column twice), a shared-memory input is conformed batch by
  batch, a Parquet input and the step's result are described and every declared column must
  have its declared type whatever the number of rows (a result column may also have a type
  the declared one widens exactly: a wider integer, `DOUBLE` for `FLOAT`, `DECIMAL(p',s)` for
  `DECIMAL(p,s)` with `p' >= p`), and the output is conformed before it is written. Decimals keep their precision, NULL stays NULL,
  and a value that would overflow or lose a digit is never rounded, truncated or cast.
- Fails with `MG-0102` when the session has the `icu` extension loaded, checked with
  `duckdb_extensions()` when the session opens, after the macros and before the result is
  read.
- Removes a shared-memory boundary file whose write failed, so that no reader finds it
  truncated.
- Keeps time zones: an input must carry its declared zone, and DuckDB's `TIMESTAMPTZ`, which
  keeps only the instant and gives it back in UTC (the port never loads `icu`), gets the
  declared zone back on the same instants, whatever the host's zone.
- Writes the declared columns even when the step returns no rows.
- Executes the built-in table macro `as_of(table, ts)` (`AS_OF`) in every segment session,
  before the project's macros: the version of each key valid at `ts`, with
  `_valid_from <= ts AND ts < _valid_to`, without tombstones (MG-REQ-0666).
- Applies one run of a history model (`apply_history`, `History`) to its history table in a
  session, in one transaction, creating the table on the first run with the model's columns
  followed by `_valid_from`, `_valid_to` (`TIMESTAMPTZ`), `_is_current` and `_is_deleted`
  (`BOOLEAN`); a current version ends at `9999-12-31 00:00:00+00` (MG-REQ-0663):
  - a key opens a version when it has no current version, or a tombstone, or when `by`
    detects a change: `ChangeDetection::Increases` for a strictly greater, non-NULL column,
    `ChangeDetection::Differs` for any listed column `IS DISTINCT FROM` the current one
    (MG-REQ-0662); the replaced version ends where the new one starts;
  - `_valid_from` is the `by` column when it is one column and not NULL, else the row's
    `_extracted_at` for a changes view, else the detection time the caller gives;
  - `Deletes::Ignore`, `Close` or `Tombstone` for a key that disappears: missing from the
    whole input, at the detection time, or, for a changes view, a row with `_deleted = true`
    at its `_deleted_at` (MG-REQ-0664);
  - from a changes view (`HistoryInput::Changes`), only the rows of `ok` loads past the last
    applied `_load_id`, each key's rows in `_extracted_at` then `_load_id` order, without the
    raw metadata columns; it returns the new highest `_load_id` (MG-REQ-0667);
  - an input that is not a changes view with a key twice fails with `MG-0608`, a
    `by = "<column>"` that is not a date or a timestamp with `MG-0609`, and a NULL key with
    `MG-0610`, all before anything is written; a `by` value not later than the current
    version's `_valid_from` gives way to the detection time, so no range is inverted, and
    a raw delete row with no `_deleted_at` uses the detection time.
- Drops a history table for a full rebuild only when the rebuild is confirmed
  (`rebuild_history`, `RebuildHistory`), or fails with `MG-0607` (MG-REQ-0665).
- Hashes a table independently of the order of its rows (`table_hash`): the SHA-256 of the
  sorted SHA-256 hashes of each row's canonical JSON, each value tagged with its Arrow type
  (MG-REQ-0638).

## What it does not

- It does not plan segments or choose a boundary's transport: `mergus-plan` does, and the
  plan records it.
- It does not wire a run: `mergus-runtime` maps the plan onto `Segment` and calls it.
- It does not read the `[resources]` section: the budget is fixed until phase 0.9.
- It does not memory-map Arrow IPC files. Reading streams them, so peak memory is one batch
  plus what DuckDB holds, with no `unsafe`.
- It does not parse `--rebuild-history` or ask for its confirmation: the CLI does, and
  passes `RebuildHistory`.
- It does not read `ops.loads` or keep the last applied `_load_id`: the caller passes the
  `ok` loads and the last applied id, and keeps the one `apply_history` returns.
- It prints nothing and keeps no state between segments.
