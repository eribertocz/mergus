# 0047 · Incremental loads, deletes, resume and late rows, made precise

- Status: accepted
- Date: 2026-11-27
- Answers: an audit of issue #198 before it started, so that its implementer does not stop on
  spec-defects

## Context

MG-REQ-0412, 0413, 0415, 0418 and 0420 leave some observable behavior open:

- which value is the watermark, and how overlap applies to columns that are not times;
- how re-read rows stay out of `__changes`;
- how the watermark is stored;
- what a committed chunk is, and where a resume starts;
- what `column:<name>` and `compare` do exactly;
- how very late rows are found and reported.

## Decision

- **Watermark (MG-REQ-0412.1):** the largest non-NULL incremental value among a load's
  committed rows.
- **Overlap (MG-REQ-0412.2):** it applies only to date and time columns. Other column types are
  read from `>= watermark`.
- **No duplicates (MG-REQ-0412.3):** a re-read row equal to the current row of its key is not
  appended to `__changes` again.
- **Stored watermark (MG-REQ-0413):** it is text, `CAST(value AS VARCHAR)`, kept in
  `ops.loads.watermark_after`.
- **Chunks (MG-REQ-0418, MG-REQ-0518.1):**
  - Each chunk is an append commit to `__changes`, under the load's `load_id`.
  - The last commit rewrites the current table. Only then is the `ops.loads` row written.
  - A `load_id` with no `ops.loads` row is unfinished. Readers ignore it, and the next
    extraction resumes it after its largest key.
  - A table without `key` is read again whole.
  - This keeps MG-REQ-0514.2: readers still use only recorded loads.
- **Delete policies (MG-REQ-0415):**
  - With no `deletes`, nothing is detected.
  - `column:<name>`: `true`, or any non-NULL value of a non-boolean column, means deleted.
  - `compare` runs when it is due, measured from `ops.loads.deletes_checked_at`. That is a new
    column of MG-REQ-0514.
- **Very late rows (MG-REQ-0420):**
  - They are found by `compare` and counted, not loaded.
  - The count is a warning with its own code.
  - The suggested `mergus backfill --source … --table … --from …` command is specified with
    raw backfill, later.

## Consequences

- A long full load commits once per chunk to `__changes`, so a crash loses at most one chunk of
  work.
- Very late rows are found only on tables with `deletes = compare`.
