# 0046 · Load registry, raw retention, raw off, and the PostgreSQL session parameters

- Status: accepted
- Date: 2026-11-27
- Answers: spec-defects #224 (issue #195) and #225 (issue #223)

## Context

#224: MG-REQ-0514 names the columns of `ops.loads` but not their types or values. MG-REQ-0516
gives a retention per source, but `expire` acts on the whole catalog. MG-REQ-0517 does not
say where `raw = false` is recorded.

#225: decision 0045 put `APPLICATION_NAME` and `OPTIONS` in the `TYPE postgres` secret next
to `URI`. The DuckDB of this repository refuses that at bind time: "Options with name
"options" cannot be specified when 'URI' option is specified".

## Decision

- **`ops.loads` (MG-REQ-0514):** its columns and types are given in the rule.
  - `load_id` is a UUID version 7, built from the ports so that tests stay deterministic.
  - Watermarks are stored as text, because their types differ from table to table.
  - A failed load is recorded too, with its code.
  - The row is committed after the raw commit, because ops can be another store. A raw
    version with no registry row is ignored: the next load starts from the last recorded
    watermark, so nothing is lost and overlap removes duplicates (MG-REQ-0412).
- **Retention (MG-REQ-0516):** retention is counted in calendar days from commit. The shared
  raw catalog is expired with the longest retention of its sources. A retention per table
  would need a new adapter operation.
- **Raw off (MG-REQ-0517):** each extraction of a source with `raw = false` records an
  `ops.loads` row with mode `raw_off`. `plan` prints `source <name>: raw = false` for those
  sources only. The plan's JSON gets the field with the plan contract, later.
- **Session parameters (MG-REQ-0429):** Mergus adds `application_name=mergus` and
  `options=-c lock_timeout=5000` to the connection value in memory, in the form of that
  value: as percent-encoded query parameters for a URI, as keywords for a keyword/value
  string. The value is never stored or logged. A value that already sets either parameter is
  refused, so Mergus never depends on how libpq treats a repeated parameter.
- **Verification:** the implementer checks these forms against the libpq page
  (https://www.postgresql.org/docs/current/libpq-connect.html) and cites it in the pull
  request. If the page contradicts them, the implementer opens a spec-defect.

## Consequences

- The connection value is changed only in memory, inside the `Database` port.
- Sources whose retention is shorter than another source's keep their raw versions longer.
