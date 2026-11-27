---
id: extract-source-care
title: Protecting the source database while extracting
crate: mergus-source
phase: "0.3"
status: draft
depends_on: [extract-loads]
---

# Source care

Mergus MUST never be the reason a source database becomes slow.

| Source option | Values | Default |
|---|---|---|
| `replica` | secret reference | none |
| `load` | `low` \| `normal` \| `high` | `low` in `prod`, `normal` elsewhere |
| `max_connections` | integer | 2 |
| `window` | `"HH:MM-HH:MM"` | none |
| `query_timeout` | duration | `10m` |

### MG-REQ-0426 · Replica first

When `replica` is set, every extraction query MUST go to the replica.

- Kind: behavior
- Verify: test `mg_req_0426_queries_go_to_replica`

### MG-REQ-0427 · Load budget

`load` MUST set the number of connections, chunk size and pause between chunks from this
table; `max_connections` MUST never be exceeded, so the connections used are the smaller of
the table's and `max_connections`:

| `load` | Connections | Chunk rows | Pause between chunks |
|---|---|---|---|
| `low` | 1 | 10 000 | 1 s |
| `normal` | 2 | 50 000 | 200 ms |
| `high` | 4 | 100 000 | none |

- Kind: behavior
- Verify: test `mg_req_0427_connections_never_exceed_budget`
- Decision: 0045

### MG-REQ-0428 · Automatic brake

Mergus MUST measure source latency per chunk; when it exceeds twice its running median it
MUST halve the pace, and restore it gradually when latency recovers. After three
consecutive failures it MUST stop the source's extraction, raise an incident and not retry
until the next trigger.

1. MG-REQ-0428.1: halving the pace doubles the pause before the next chunk, starting from the
   chunk's own latency when the budget has no pause;
2. MG-REQ-0428.2: each chunk whose latency is at most the running median halves the extra
   pause, until the pause is back to the budget's;
3. MG-REQ-0428.3: a failure is a chunk statement that fails or times out; a slow chunk is
   not a failure;
4. MG-REQ-0428.4: in phase 0.3 the incident is a typed error (`MG-04xx`) naming the source
   and the last failure, reported by the run; the incident contract of `ops/` replaces it
   when it arrives.

- Kind: behavior
- Verify: test `mg_req_0428_brake_slows_and_stops`
- Decision: 0045

### MG-REQ-0429 · Protected session

Every source session MUST be read-only, set a statement timeout and a lock timeout, and
identify itself as `mergus` (for example `application_name` in PostgreSQL).

1. MG-REQ-0429.1: the lock timeout is 5 seconds;
2. MG-REQ-0429.2: for PostgreSQL, read-only is the `READ_ONLY` attach and the statement
   timeout is `pg_statement_timeout_millis` (MG-REQ-0196.2, MG-REQ-0196.3); the
   identification and the lock timeout are the libpq parameters `application_name=mergus`
   and `options=-c lock_timeout=5000`, added in memory to the connection value before it
   goes into the temporary secret: as percent-encoded query parameters of a URI, or as
   keywords of a keyword/value string;
3. MG-REQ-0429.3: a connection value that already sets `application_name` or `options` is
   refused with a typed error (`MG-04xx`) naming the source and the parameter, never its
   value.

- Kind: behavior
- Verify: test `mg_req_0429_session_is_read_only_and_identified`
- Decision: 0045, 0046

### MG-REQ-0430 · Time window

When `window` is set, full loads and backfills of the source MUST start only inside the
window; incremental loads are not restricted.

- Kind: behavior
- Verify: test `mg_req_0430_full_loads_wait_for_window`

### MG-REQ-0431 · Index check

`mergus doctor --source <name>` MUST warn when the incremental column or the key of a table
has no index in the source, with the suggested index.

1. MG-REQ-0431.1: the warning has its own `MG-04xx` code and severity warning; it never
   changes the exit code;
2. MG-REQ-0431.2: the suggested index is the statement
   `CREATE INDEX ON "<schema>"."<table>" ("<column>", …);`, with the key columns in key
   order, or the incremental column alone;
3. MG-REQ-0431.3: in phase 0.3 only PostgreSQL sources are checked, reading `pg_index`
   through `postgres_query`; a DuckDB file source is not checked.

- Kind: behavior
- Verify: test `mg_req_0431_doctor_reports_missing_index`
- Decision: 0045

### MG-REQ-0432 · Source impact in the plan

`mergus plan` MUST show, per source table: estimated rows to read, the index used when known,
the window, and the load budget.

- Kind: behavior
- Verify: test `mg_req_0432_plan_shows_source_impact`
