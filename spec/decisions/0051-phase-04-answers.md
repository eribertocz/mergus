# 0051 · Phase 0.4: answers given before the issues start

- Status: accepted
- Date: 2026-11-27
- Answers: an audit of the 57 phase 0.4 rules before their issues were filed, so that the
  implementers do not stop on spec-defects. Each rule below holds as written, and this
  decision gives the values the rule leaves open.

## Python (MG-REQ-0006, 0611-0619, 1003)

- **The `Python` port:** a new port in `mergus-ports`, with its simulated version in `mergus-sim`.
  - The real port runs the interpreter as a child process, with the environment of
    MG-REQ-1003.1. It never embeds Python in the Mergus process.
  - The interpreter is `[python] interpreter`, else `<root>/.venv`, else `python3` on `PATH`.
    If none is found, the step fails with a typed error.
- **Reading without importing (MG-REQ-0611):** `python -I -S` runs a bundled `ast` script.
  - The script returns a Mergus JSON tree of the file: functions, parameters, returns,
    imports, calls, and their lines and columns.
  - User code is never imported.
- **Return kinds (MG-REQ-0613):**
  - A step that returns a plain string literal, or literals joined implicitly, is SQL.
  - An f-string, `+`, `.format` or a variable as the return is a validation error.
  - Any other return is Polars, confirmed when the step runs (MG-REQ-0617).
  - Every return of a step must have the same kind.
- **Eager calls (MG-REQ-0616):** these are validation errors in a step: `.collect`,
  `.collect_async`, `.fetch`, `.to_pandas`, `.to_arrow`, `.to_dicts` and `pl.DataFrame(`.
- **Errors at read (MG-REQ-0702):** they are grouped by file, in layout order.
- **Server mode (MG-REQ-0636):** it does not exist in phase 0.4.
  - In 0.4 the test covers the allowlisted environment and the work directory.
  - The network and file sandbox of server mode is tested in the phase that adds server mode.
- **Tests:** they need Python and Polars in CI, pinned with `uv`. Their versions enter the
  fingerprint's `engines`.

## SQL, macros and seeds (MG-REQ-0601-0604, 0641-0646)

- **Run parameters:** `$run_date` (a DATE, the run's UTC start or `--run-date`),
  `$period_start` and `$period_end`. Any other name is a validation error.
- **Macros:** a macro file may contain only `CREATE MACRO`/`CREATE FUNCTION` statements, which
  `extract_statements` checks. It is then run in an in-memory DuckDB with external access off,
  and the macros are read from `duckdb_functions()`. A built-in is an entry with
  `internal = true`.
- **Seeds:**
  - Column types are DuckDB type names.
  - A CSV with no declaration, or a declaration with no file, is an error.
  - "Small" means at most 10,000,000 bytes; a larger seed is a warning.
  - A seed's hash is the SHA-256 of its file bytes.

- **Schemas of steps with run parameters (MG-REQ-0603, 0604):** DuckDB cannot type a query
  whose parameters are unbound.
  - Each run parameter has a fixed type: `$run_date` is a DATE, and `$period_start` and
    `$period_end` are TIMESTAMPTZ.
  - The schema is obtained by binding a typed placeholder for each parameter and running the
    query against empty tables with the input schemas. No data exists, so none is read.
- **SQL model errors (MG-REQ-0601-0604):** each is a validation error with its own `MG-06xx`
  code:
  - a model that parses but that DuckDB refuses to prepare (an unknown column, a type mismatch,
    a binder error);
  - an unknown parameter name;
  - a positional parameter (`?`, `$1`).

## Engines (MG-REQ-0180, 0631-0638)

- **Fusion (MG-REQ-0631):** steps are fused only inside one model. Each `.sql` model is one
  segment.
- **Polars levels (MG-REQ-0635):** the native and in-process levels never qualify in 0.4. The
  plan records that reason.
- **Memory budget (MG-REQ-0633):** 1 GiB of Arrow per boundary, until `[resources]` arrives in
  phase 0.9.
  - "Shared memory" means an Arrow IPC file in the run's work directory.
  - It is written and read one record batch at a time, through streaming readers and writers
    of the `FileSystem` port. Peak memory therefore holds one batch, never a second copy of
    the whole boundary.
  - Files are not memory-mapped. Mapping needs `unsafe`, which MG-REQ-0170 keeps to FFI
    crates, and streaming meets the budget without it (#254).
  - The simulated port streams over its stored bytes.
  - The plan records the transport (MG-REQ-0747), and execution applies it.
- **Table hash (MG-REQ-0638):** the SHA-256 of the sorted SHA-256 hashes of each row's
  canonical JSON.
- **Type passport (MG-REQ-0634, 0180):**
  - The DuckDB to Polars type map is lossless.
  - These types block a Polars boundary with a typed error: `HUGEINT`, `UHUGEINT`, `UNION`,
    `BIT`, `INTERVAL`, and `DECIMAL` above 38 digits.
- **Non-determinism tokens (MG-REQ-0637):** `limit_without_order`, `any_value`, `random`,
  `now`, `unique_without_keep` and `first_without_sort`. They are checked only on published
  models.

## Refresh and history (MG-REQ-0651-0657, 0661-0667)

- **Folder options:** `by` is a column name or a list of them. `deletes` defaults to `ignore`.
- **Affected periods (MG-REQ-0652):** a period is affected when a row of a changes view falls
  in it. A dimension input marks every period.
- **History detection (MG-REQ-0662, 0663):**
  - Detection time is the run's UTC start, to the microsecond.
  - "Increases" means strictly greater, and a NULL never opens a version.
  - The first version's `_valid_from` is the `by` value when `by` is one column, otherwise the
    detection time.
- **Rebuilding history (MG-REQ-0665):** `--rebuild-history` applies to `run --full-refresh` and
  to `backfill`. Without a terminal and without `--yes`, it fails.
- **Backfill (MG-REQ-0657):** in 0.4 it recomputes into work only. Atomic publication comes
  with phase 0.5.

## Stages, gates and limits (MG-REQ-0503, 0701-0722, 1313)

- **Layers:** the gates live in `mergus-plan`. The credentials gate takes a precomputed
  existence map, built by `mergus-security`'s existence check. That check uses environment
  variable presence and `Keychain::exists`, and never reads a value.
- **Gates of later phases:** they report `not_applicable` with their phase.
- **Gate errors:** each one carries the gate, the model, the step, the code and the fix.
- **The work zone (MG-REQ-0503 and 0707):** the two rules are reconciled.
  - `.mergus/work/runs/<run_id>/` is scratch. It is deleted at the end of the run and at the
    next start.
  - `.mergus/work/candidates/<fingerprint>/` is kept for resuming. A candidate that no plan
    references is pruned at the start of the next run.
- **Candidates (MG-REQ-0705):** "invisible" means attached to no catalog and listed by no
  command.
- **Cancellation (MG-REQ-0708):** a cancel token that the CLI sets on Ctrl-C. A cancelled run
  ends with exit code 7.
- **Limits (MG-REQ-0718):**
  - The limits are `[limits] wall_time`, `rows_written` and `bytes_scanned`, or the `--max-*`
    flags. When both are set, the stricter wins.
  - Bytes scanned are summed from DuckLake file sizes before execution. Rows are checked per
    batch.
- **Intermediates (MG-REQ-0656):** an intermediate model (`publish = false`) is a view inside
  the session and is never written.
- **The finalizer (MG-REQ-0709):** the run report is a `run_finished` event. Notifications are
  a no-op until phase 0.7.
- **Idempotency keys (MG-REQ-1313):**
  - They are stored in `.mergus/idempotency/<sha256(key)>.json`, with the hash of the
    canonical arguments, the exit code and the output, for 24 hours.
  - The same key with other arguments is an error.
  - They apply to the commands that change state: `init`, `new`, `add source`, `secret set`,
    `mv`, `run` and `backfill`.
- **Error codes:** each error takes the next free code of its area when its pull request
  merges.

## Phase 0.3 follow-up (spec-defect #231)

- **`delete_window` (MG-REQ-0415.3):** it counts back from the load's `_extracted_at`.
- **The late-row warning (MG-REQ-0420.2):** `<since>` and `<oldest>` are written as DuckDB's
  `CAST(value AS VARCHAR)` text, the same text as `watermark_after` (MG-REQ-0413).

## Consequences

The implementers of phase 0.4 build from these values. A value that proves wrong is changed
here, through a specification pull request, before code depends on it.
