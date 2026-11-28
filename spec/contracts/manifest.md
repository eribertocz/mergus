---
id: contracts-manifest
title: Publication manifest, pointer and catalog layout
crate: mergus-catalog
phase: "0.5"
status: draft
depends_on: [contracts-overview, MG-REQ-0140]
---

# Manifest and pointer

## 1. Layout

```
catalog/
  <encoded origin>/
    schema/<snapshot>.json
    state/<snapshot>.json
  manifests/<manifest_id>.json
  envs/<environment>/latest
```

## 2. Manifest fields

| Field | Content |
|---|---|
| `contract`, `contract_version` | `mergus.manifest`, major version (MG-REQ-0204) |
| `manifest_id` | Unique identifier |
| `environment` | Environment name |
| `previous_manifest_id` | The manifest it replaces; null for the first |
| `schema_document_id`, `state_document_id` | The catalog documents it publishes |
| `published_at`, `readable_until` | Publication time and rollback-window end, RFC 3339 UTC (MG-REQ-0255) |
| `catalogs[]` | One entry per DuckLake catalog the tables are read from (MG-REQ-0258) |
| `tables[]` | One entry per gold, `connect` and `live` table (MG-REQ-0253) |
| `model_hash` | Ossie model hash the catalog was validated against |
| `run_id` | Run that produced it |
| `mergus_version` | Version that produced it |
| `previous_hash`, `hash` | Hash chain (RFC 8785, SHA-256) |

### MG-REQ-0245 · Immutable documents

Catalog documents and manifests MUST be stored under their identifier and never overwritten.
They MUST NOT be deleted while referenced by a pointer or inside the rollback window.

- Kind: behavior
- Verify: test `mg_req_0245_documents_never_overwritten`

### MG-REQ-0246 · Atomic writes

Every document MUST be written atomically: temporary file plus rename on local disks, one
object write on object stores.

- Kind: behavior
- Verify: test `mg_req_0246_document_writes_are_atomic`

### MG-REQ-0247 · Publication order

A publication MUST write, in this order: the new `catalog_schema` if the structure changed,
the `catalog_state`, the manifest, and finally the pointer.

- Kind: behavior
- Verify: test `mg_req_0247_publication_write_order`

### MG-REQ-0248 · Pointer compare-and-swap

The pointer `envs/<environment>/latest` MUST be moved with compare-and-swap against the
`previous_manifest_id`. If another writer moved it first, the run MUST end with exit code 4
and no visible effect (INV-08).

- Kind: behavior
- Verify: test `mg_req_0248_pointer_moves_by_compare_and_swap`
- Depends: MG-REQ-0147

### MG-REQ-0249 · Self-describing pin recipe

Each table entry MUST contain the pin recipe returned by its adapter (MG-REQ-0298), so that a
reader can read the exact version from the manifest alone.

- Kind: behavior
- Verify: test `mg_req_0249_manifest_alone_pins_every_table`
- Depends: MG-REQ-0298
- Decision: 0043

### MG-REQ-0251 · Data token per table

Each table entry MUST carry `data_token`, which changes whenever the table's data changed
since the previous manifest and stays equal when the table was not written:

1. MG-REQ-0251.1: levels A, A-ext and B: the native version at which the table last changed,
   not the newest version of the catalog;
2. MG-REQ-0251.2: levels C and C+: the identifier of the publication that last wrote the
   table, valid because only publication writes gold (INV-02);
3. MG-REQ-0251.3: level F (`live`): `null`, and the entry is marked live.

A rewrite with identical rows MAY change the token; equal tokens always mean equal data.

- Kind: behavior
- Verify: test `mg_req_0251_data_token_changes_only_with_data`
- Depends: MG-REQ-0911
- Decision: 0034

### MG-REQ-0252 · Pinned versions stay readable

Every version that a manifest inside the rollback window references MUST stay readable at
that version. Reading a version that has expired, or that does not exist, MUST fail with a
typed error naming the table and the version, never return other data:

1. MG-REQ-0252.1: when Mergus itself reads a pinned version (`diff`, `audit`, `rollback`), the
   error has its own next free `MG-02xx` code for an expired version and another for one that
   never existed, both
   exit code 1. A consumer reading the storage directly gets the storage's own error; it uses
   `readable_until` (MG-REQ-0255) to refuse such reads before it makes them.

- Kind: behavior
- Verify: test `mg_req_0252_expired_version_fails_typed`
- Depends: MG-REQ-0245, MG-REQ-0288
- Decision: 0034

### MG-REQ-0250 · Manifest hash chain

Each manifest MUST carry the hash of its predecessor and its own hash, so that
`mergus audit verify` can verify the chain of an environment.

- Kind: behavior
- Verify: test `mg_req_0250_manifest_chain_verifies`

### MG-REQ-0253 · Manifest schema

The manifest MUST validate against the published JSON Schema
`schemas/mergus_manifest/v1/schema.json`, which ships with valid and invalid examples and
follows contract versioning (MG-REQ-0204). Each `tables[]` entry has exactly:

1. MG-REQ-0253.1: `name` (logical), `physical`, `level` (`A`, `A-ext`, `B`, `C`, `C+`, `F`),
   `engine` and `location` (MG-REQ-0254), `encrypted`, `data_version`, `data_token`
   (MG-REQ-0251), `row_count` (null for `live`), `live` (boolean), `freshness`
   (MG-REQ-0259).
2. MG-REQ-0253.2: `pin`, the recipe of MG-REQ-0249: an object with `kind` and its fields:
   `ducklake_snapshot` {`snapshot_id`}, `iceberg_snapshot` {`snapshot_id`}, `delta_version`
   {`version`}, `parquet_files` {`files`: paths relative to `location`}, `connect` {`version`
   or null}, `clickhouse_table` {`table`} (MG-REQ-0244), `live` {}.
3. MG-REQ-0253.3: `quality` {`status`: `passed`, `warned` or `failed`; `passed`, `warned`,
   `failed`: counts of tests} and `health` {`status`: `healthy`, `stale` or `degraded`;
   `reason`: text or null}.
4. MG-REQ-0253.4: the pointer `envs/<environment>/latest` is a JSON document {`manifest_id`,
   `moved_at`}, validated by `schemas/mergus_manifest/v1/latest.schema.json`.

- Kind: behavior
- Verify: test `mg_req_0253_manifest_validates_against_schema`
- Decision: 0054

### MG-REQ-0254 · Engine and logical location

Each table entry MUST name its `engine` (`ducklake`, `iceberg`, `delta`, `parquet`, or for a
`connect` or `live` table the source's engine, such as `postgres`) and a logical `location`: the
catalog URL, path or object-store prefix the table is read from. A location MUST NOT contain a
credential, a secret reference or a query parameter that carries one.

- Kind: behavior
- Verify: test `mg_req_0254_location_has_no_credentials`
- Decision: 0054

### MG-REQ-0255 · Readable until

Each manifest MUST carry `readable_until`: its publication time plus the environment's rollback
window (MG-REQ-0904). Every version it pins stays readable at least until then; a later change
of the window never shortens the guarantee of a manifest already published.

- Kind: behavior
- Verify: test `mg_req_0255_readable_until_is_kept`
- Decision: 0054

### MG-REQ-0256 · Withdrawn manifests

A rollback (MG-REQ-1226) MUST write `envs/<environment>/withdrawn/<manifest_id>.json` for each
manifest it withdraws, before it moves the pointer, with {`manifest_id`, `withdrawn_at`,
`replaced_by`, `reason`}. Manifests stay immutable; the marker is the signal.

- Kind: behavior
- Verify: test `mg_req_0256_rollback_marks_withdrawn`
- Phase: 0.7
- Decision: 0054

### MG-REQ-0257 · Documents are kept

Manifests and catalog documents (`schema` and `state`) MUST never be deleted, so the hash chain
verifies (MG-REQ-0250) and an audited query's SQL can be regenerated from the schema of its
time. Only table data versions expire, after the rollback window.

- Kind: behavior
- Verify: test `mg_req_0257_documents_never_deleted`
- Decision: 0054

### MG-REQ-0258 · One catalog version per DuckLake catalog

A reader pins a DuckLake catalog as a whole when it attaches it, so the manifest MUST give one
snapshot per DuckLake catalog at which every table entry of that catalog holds:

1. MG-REQ-0258.1: `catalogs[]` has one entry {`engine`: `ducklake`, `location`, `snapshot_id`}
   per distinct DuckLake `location` among `tables[]`, gold and `connect` alike.
2. MG-REQ-0258.2: a publication commits all the gold tables it writes in one DuckLake snapshot.
   The gold entry's `snapshot_id` is that snapshot, or the previous manifest's when the
   publication wrote no gold table.
3. MG-REQ-0258.3: for an external DuckLake catalog, stage 2 pins one snapshot per catalog, and
   every `connect` table of that catalog is checked (MG-REQ-0912) and read at it.
4. MG-REQ-0258.4: the `pin.snapshot_id` of every table of a catalog equals that catalog's
   `snapshot_id`. A table's `data_token` is the snapshot at which it last changed, which is a
   catalog snapshot, never newer than `snapshot_id` (MG-REQ-0251.1).

- Kind: behavior
- Verify: test `mg_req_0258_one_snapshot_per_ducklake_catalog`
- Depends: MG-REQ-0251, MG-REQ-0253
- Decision: 0055

### MG-REQ-0259 · Freshness per table

Each table entry MUST carry `freshness` {`data_as_of`, `fresh`}, so that a reader shows how
current a table is without querying it:

1. MG-REQ-0259.1: `data_as_of`: the `_extracted_at` of the newest raw load that the table's
   rows were built from, RFC 3339 UTC. It is null for `connect` and `live` tables, whose loads
   Mergus does not see.
2. MG-REQ-0259.2: `fresh`: the model's `fresh` target as declared (such as `6h`), or null.
3. MG-REQ-0259.3: a reader computes the age as its own clock minus `data_as_of`. At
   publication, `health.status` is `stale` when that age already exceeds `fresh`
   (MG-REQ-0732).

- Kind: behavior
- Verify: test `mg_req_0259_freshness_in_manifest`
- Depends: MG-REQ-0253
- Decision: 0055

### MG-REQ-0244 · ClickHouse tables are pinned by name

A ClickHouse gold table (level C) MUST be readable at the version a manifest pins:

1. MG-REQ-0244.1: each publication writes the table's rows to a new physical table
   `<physical>__<data_token>`, which is never changed afterwards. Then it moves the stable name
   `<physical>` onto it in one atomic exchange.
2. MG-REQ-0244.2: the entry's `pin` is `clickhouse_table` {`table`}, the physical table of the
   manifest's version. A reader reads that table, so no check before and after the read is
   needed. The stable name always serves the newest publication.
3. MG-REQ-0244.3: a physical table is dropped only once no manifest inside the rollback window
   references it (MG-REQ-0255).

- Kind: behavior
- Verify: test `mg_req_0244_clickhouse_pinned_by_table`
- Phase: 1.5
- Depends: MG-REQ-0251
- Decision: 0055
