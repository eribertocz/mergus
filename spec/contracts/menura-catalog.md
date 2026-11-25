---
id: contracts-menura-catalog
title: Producing catalog_schema and catalog_state for Menura
crate: mergus-catalog
phase: "0.5"
status: draft
depends_on: [contracts-overview]
---

# Menura catalog

Menura owns these contracts; its specification (`spec/contracts/catalog.md`) is the source of
truth. Mergus is the producer. Where this file and Menura's specification differ, Menura's
wins and this file is a defect.

| Document | Contains | Menura uses it from |
|---|---|---|
| `catalog_schema` | Relations, columns, logical types, nullability | 0.2.0 |
| `catalog_state` | Versions per relation and period, summary builds, check results | 0.6.0 |

### MG-REQ-0210 · Documents follow Menura's schema

Mergus MUST emit both documents valid against the JSON Schemas of the Menura release it is
pinned to, selected by `contract` and `contract_version`, never by `$id` URL. Validation runs
at both levels (schema and Menura's semantic rules CAT-8) before publication.

- Kind: behavior
- Verify: test `mg_req_0210_catalog_valid_against_pinned_menura`

### MG-REQ-0211 · Document identity

Each document MUST carry `document_id = {origin, snapshot}`. `origin` is stable ASCII naming
the storage and environment (for example `ducklake:prod`). `snapshot` is the real storage
version where the data was last written. A new document always gets a new `document_id`.

- Kind: behavior
- Verify: test `mg_req_0211_document_id_is_origin_and_storage_snapshot`

### MG-REQ-0212 · Producer guarantees

Mergus MUST keep Menura's producer guarantees CAT-7.1.1 to CAT-7.1.5: a token is never reused
for different data; `schema_document_id` names exactly the schema the state is consistent
with; `aggregate_build.definition_hash` is the hash Menura computed; `integrity_check`
reports the result of the query Menura generated over the versions in `verified`;
`partitions.versions` lists every period with rows.

- Kind: behavior
- Verify: test `mg_req_0212_producer_guarantees_hold_across_publications`

### MG-REQ-0213 · Tokens per period

A change to any row of a period, including a partial correction, MUST produce a new token for
that period. A period without changes MUST keep its token. A period left without rows MUST
leave the list.

- Kind: behavior
- Verify: test `mg_req_0213_period_tokens_track_row_changes`
- Depends: MG-REQ-0142

### MG-REQ-0218 · Relation versions are the data tokens

Versions in `catalog_state` MUST come from the manifest's data tokens (MG-REQ-0251), never
from the snapshot of the whole catalog:

1. MG-REQ-0218.1: a relation that is not partitioned carries `version`, and its
   `version.snapshot` MUST equal the table's `data_token`;
2. MG-REQ-0218.2: a partitioned relation carries `partitions`, not `version`, with the period
   tokens of MG-REQ-0213, and the table's `data_token` MUST change if and only if the token of
   some period changes.

- Kind: behavior
- Verify: test `mg_req_0218_relation_versions_are_data_tokens`
- Depends: MG-REQ-0213, MG-REQ-0251
- Decision: 0034

### MG-REQ-0214 · Hashes are copied, never computed

Hashes that Menura defines (`definition_hash`, `model_hash`) MUST be copied verbatim from
Menura's documents. Mergus MUST NOT compute them.

- Kind: behavior
- Verify: test `mg_req_0214_menura_hashes_copied_verbatim`

### MG-REQ-0215 · Type mapping

Storage types MUST map to catalog logical types as follows; any other type maps to
`unsupported` and Mergus warns when a gold model uses it.

| Storage type | Logical type |
|---|---|
| TINYINT, SMALLINT, INTEGER, BIGINT, HUGEINT (and unsigned) | `integer` 8/16/32/64/128, signed or unsigned |
| DECIMAL(p,s) | `decimal(p,s)` |
| FLOAT, DOUBLE | `float` 32, 64 |
| VARCHAR, BLOB, UUID, BOOLEAN | `text`, `binary`, `uuid`, `boolean` |
| DATE, TIME, INTERVAL | `date`, `time`, `interval` |
| TIMESTAMP, TIMESTAMP_S/_MS/_NS | `timestamp` us, s, ms, ns, without zone |
| TIMESTAMPTZ | `timestamp` us, with zone |
| LIST of a mappable type | `list(element)` |

- Kind: behavior
- Verify: test `mg_req_0215_type_mapping_table`

### MG-REQ-0216 · Nullability is honest

`nullable` MUST be `true` unless Mergus enforces NOT NULL on the column; when declared NOT
NULL, Mergus MUST guarantee it before publication.

- Kind: behavior
- Verify: test `mg_req_0216_not_null_enforced_when_declared`

### MG-REQ-0217 · Contract names follow Menura's rename

Until Menura releases its alias version, Mergus MUST emit `semantic_compiler.*` names; from
that release, only `menura.*`. The switch MUST be a change of the constants of
MG-REQ-0207 and the CI matrix MUST test against the current and the next Menura release.

- Kind: behavior
- Verify: test `mg_req_0217_contract_names_from_constants`
