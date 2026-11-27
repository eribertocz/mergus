# 0049 · Incremental loads: points left open, found in the review of #198

- Status: accepted
- Date: 2026-11-27
- Answers: the review of pull request #234 (issue #198)

## Decision

- **Repeated keys in a chunked load (MG-REQ-0518.3):** a key repeated across the chunks of one
  load fails the whole load. The load is recorded `failed`, and readers use only loads recorded
  `ok` (MG-REQ-0518.1), so the chunks it already committed stay invisible.
- **Watermark (MG-REQ-0412.1):** a load with no non-NULL incremental value keeps the previous
  watermark. After the first load, rows whose incremental value is NULL are re-read only by a
  full load.
- **Overlap on a `DATE` column (MG-REQ-0412.2):** it subtracts whole days, rounded up, so a 1h
  overlap re-reads the previous day.
- **Last compare (MG-REQ-0415.3):** it is the latest non-NULL `deletes_checked_at` of the table.
  A load that ran no compare stores NULL; the old value is never carried forward.
- **Invalid delete options (MG-REQ-0415.4):** `compare` without `key`, and `delete_window`
  without a date or time `incremental`, are validation errors. A delete detected for a key
  that is not current is ignored.
- **Crash after the last commit (MG-REQ-0418.2):** if a crash comes after the last raw commit,
  the next extraction only writes the missing `ops.loads` row.

## Consequences

- Silent no-ops become validation errors that name the table and say how to fix it.
