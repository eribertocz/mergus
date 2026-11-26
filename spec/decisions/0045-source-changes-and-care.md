# 0045 · Source changes and source care, made precise

- Status: accepted
- Date: 2026-11-26
- Answers: spec-defects #219 (issue #199) and #220 (issue #200)

## Context

Issues #199 and #200 found undefined behavior:

- MG-REQ-0405: what extraction does for `connect` and `live` sources.
- MG-REQ-0407: what an incompatible type change or a rename is.
- MG-REQ-0419: how read times and the spread are measured.
- MG-REQ-0427: the load budget table.
- MG-REQ-0428: the brake's step, what a failure is, and what an incident is.
- MG-REQ-0429: the lock timeout and how a PostgreSQL session is identified.
- MG-REQ-0431: the index warning.

Facts the implementers checked, from official pages:

- The DuckDB `postgres` extension has an `ISOLATION_LEVEL` attach option, but no page says that
  several attached tables share one snapshot
  (https://duckdb.org/docs/2.0/core_extensions/postgres/overview.html).
- It has no lock-timeout setting and no `application_name` setting. Its `TYPE postgres` secrets
  take `APPLICATION_NAME` and `OPTIONS`, keys "named after the corresponding connection options
  of libpq" (https://duckdb.org/docs/2.0/core_extensions/postgres/secrets.html).
- libpq documents `application_name`, and `options` with `-c`
  (https://www.postgresql.org/docs/current/libpq-connect.html#LIBPQ-PARAMKEYWORDS).
- PostgreSQL documents `lock_timeout`
  (https://www.postgresql.org/docs/current/runtime-config-client.html#GUC-LOCK-TIMEOUT).

## Decision

- **`connect` and `live` sources (MG-REQ-0405):** extraction returns no table and reads
  nothing. This is not an error.
- **Type changes (MG-REQ-0407):** in phase 0.3 every type change stops the table. Allowing a
  widening needs raw to widen its column too, which is schema evolution, a later phase.
- **Renames:** a missing column is reported as "dropped or renamed". Renames are not guessed,
  since a guess can be wrong.
- **Added columns** do not stop the table.
- **Read times (MG-REQ-0419):** each table's read has a start (its first statement) and an end
  (its last). The spread is the latest end minus the earliest start, so it shows the whole
  window the data may come from. PostgreSQL in phase 0.3 records times, because no official
  page documents a shared snapshot through the extension.
- **Load budget (MG-REQ-0427):** the table of `low`, `normal` and `high` is in the
  specification. The connections used are the smaller of the table's and `max_connections`.
- **Brake (MG-REQ-0428):**
  - A slow chunk doubles the next pause. Each chunk at or under the median halves the extra
    pause again.
  - Only a failed or timed-out statement counts as a failure.
  - In phase 0.3 the incident is a typed `MG-04xx` error. The `ops/` incident contract takes
    over later.
- **Protected session (MG-REQ-0429):** the lock timeout is a fixed 5 seconds. The `Database`
  port puts `APPLICATION_NAME 'mergus'` and `OPTIONS '-c lock_timeout=5000'` in the temporary
  secret next to `URI`. If the real port shows that DuckDB does not combine these keys with
  `URI`, the implementer opens a spec-defect instead of rewriting the connection value.
- **Index check (MG-REQ-0431):**
  - The warning has its own `MG-04xx` code and never changes the exit code.
  - The suggestion is a `CREATE INDEX` statement.
  - In phase 0.3 only PostgreSQL is checked, through `pg_index`.

## Consequences

- A harmless widening blocks a table until schema evolution arrives; the error names the
  column and both types.
- MG-REQ-0429 changes the `Database` port, so it moves out of #200 to an issue of its own in
  `mergus-ports`.
