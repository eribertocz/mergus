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

`load` MUST set the number of connections, chunk size and pause between chunks from a fixed
table in this crate's documentation; `max_connections` MUST never be exceeded.

- Kind: behavior
- Verify: test `mg_req_0427_connections_never_exceed_budget`

### MG-REQ-0428 · Automatic brake

Mergus MUST measure source latency per chunk; when it exceeds twice its running median it
MUST halve the pace, and restore it gradually when latency recovers. After three
consecutive failures it MUST stop the source's extraction, raise an incident and not retry
until the next trigger.

- Kind: behavior
- Verify: test `mg_req_0428_brake_slows_and_stops`

### MG-REQ-0429 · Protected session

Every source session MUST be read-only, set a statement timeout and a lock timeout, and
identify itself as `mergus` (for example `application_name` in PostgreSQL).

- Kind: behavior
- Verify: test `mg_req_0429_session_is_read_only_and_identified`

### MG-REQ-0430 · Time window

When `window` is set, full loads and backfills of the source MUST start only inside the
window; incremental loads are not restricted.

- Kind: behavior
- Verify: test `mg_req_0430_full_loads_wait_for_window`

### MG-REQ-0431 · Index check

`mergus doctor --source <name>` MUST warn when the incremental column or the key of a table
has no index in the source, with the suggested index.

- Kind: behavior
- Verify: test `mg_req_0431_doctor_reports_missing_index`

### MG-REQ-0432 · Source impact in the plan

`mergus plan` MUST show, per source table: estimated rows to read, the index used when known,
the window, and the load budget.

- Kind: behavior
- Verify: test `mg_req_0432_plan_shows_source_impact`
