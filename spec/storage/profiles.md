---
id: storage-profiles
title: Storage profiles and guarantee levels
crate: mergus-runtime
phase: "0.3"
status: draft
depends_on: [storage-zones, contracts-adapter]
---

# Profiles

| `profile` | For | Gold is |
|---|---|---|
| `local` | Development on one machine | DuckLake in a folder |
| `shared` | Several processes on one machine | DuckLake with a SQLite catalog |
| `cloud` | Production | DuckLake with a PostgreSQL catalog on S3, GCS or Azure |
| `server` | A DuckLake catalog served over Quack, reached as a client, from phase 0.7 (decision 0036) | DuckLake (level A); a DuckDB file through the server is level C+, phase 1.2 |

| Guarantee | Means | When |
|---|---|---|
| Verified | Pinned versions, rollback, audit per version, summaries | `copy` or `connect` over a versioned format |
| Correct | Always correct, no rollback or per-version audit | `copy` over an unversioned store |
| Live | Correct and current, no summaries | `live` mode |

### MG-REQ-0521 · Profiles are complete defaults

Each profile except `server`, which comes in phase 0.7, MUST configure catalog, data
location and concurrency of a zone (`[raw]`, `[gold]`, `[ops]`) from its `location` alone, as
DuckLake attach strings:

| `profile` | Catalog | Data files | Clients |
|---|---|---|---|
| `local` | DuckDB file `<location>/catalog.ducklake` (`ducklake:<file>`) | `<location>/data/` | one process |
| `shared` | SQLite file `<location>/catalog.sqlite` (`ducklake:sqlite:<file>`) | `<location>/data/` | several local processes |
| `cloud` | PostgreSQL, from the secret reference `catalog` (`ducklake:postgres:<value>`) | `<location>`, an object-store URL | several, remote |

1. MG-REQ-0521.1: `local` and `shared` need only `location`, a local directory; `cloud` needs
   `location` and `catalog`, the one credential it cannot default;
2. MG-REQ-0521.2: advanced users MAY override `catalog` (a path for `local` and `shared`, a
   secret reference for `cloud`) and `data_path`; any other key is an error (MG-REQ-0361);
3. MG-REQ-0521.3: the `sqlite` extension is installed only for a `shared` catalog, and the
   `postgres` extension for a `cloud` catalog (MG-REQ-0194).

- Kind: behavior
- Verify: test `mg_req_0521_profile_needs_no_more_keys`
- Decision: 0042

### MG-REQ-0522 · Guarantee per table

Validation MUST compute and show, for each gold table, its guarantee level, and the reason
when it is not Verified.

- Kind: behavior
- Verify: test `mg_req_0522_guarantee_level_per_table`
- Phase: 0.5
- Decision: 0042

### MG-REQ-0523 · Format decides the level

The guarantee level MUST be the lower of what the storage format provides and what the
executor can pin. Location never changes the level. A table the executor cannot read MUST be
rejected with the rule and the fix.

- Kind: behavior
- Verify: test `mg_req_0523_level_is_min_of_format_and_executor`
- Phase: 0.5
- Decision: 0042

### MG-REQ-0524 · Concurrency needs a multi-client catalog

When Mergus and the executor run concurrently, the DuckLake catalog MUST support several
clients (SQLite on one machine, PostgreSQL remote). A single-client catalog in that setup
MUST be a validation error.

- Kind: behavior
- Verify: test `mg_req_0524_single_client_catalog_rejected_when_concurrent`
- Phase: 0.5
- Decision: 0042

### MG-REQ-0525 · Never a wrong number

Whatever the guarantee level, a reader MUST never get a result mixing two data versions of
one table.

- Kind: behavior
- Verify: test `mg_req_0525_no_mixed_versions_at_any_level`
- Phase: 0.5
- Decision: 0042
