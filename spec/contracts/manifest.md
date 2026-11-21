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
| `tables[]` | Per gold table: logical name, physical table, data version, pin recipe, quality summary, health, row count |
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

Each table entry MUST contain the pin recipe returned by its adapter, so that a reader can
read the exact version from the manifest alone.

- Kind: behavior
- Verify: test `mg_req_0249_manifest_alone_pins_every_table`

### MG-REQ-0250 · Manifest hash chain

Each manifest MUST carry the hash of its predecessor and its own hash, so that
`mergus audit verify` can verify the chain of an environment.

- Kind: behavior
- Verify: test `mg_req_0250_manifest_chain_verifies`
