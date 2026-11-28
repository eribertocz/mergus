# 0054 · The manifest serves consumers such as Accipiter

- Status: accepted
- Context for: phase 0.5 (`contracts/manifest.md`), 0.6 (identities), 0.7 (rollback)

## Context
Accipiter, the service that will query gold, asked for eight things before the manifest is
implemented. Points 1 and 2 block it.

## Decision
1. **Schema (0.5):** a published JSON Schema `mergus_manifest/v1` with examples; exact
   `tables[]` entry, pin recipe per kind, quality and health shapes, and the `latest` pointer
   document (MG-REQ-0253). It is the first issue of phase 0.5.
2. **Engine and location (0.5):** every entry, `connect` and `live` included, names its engine
   and a logical location without credentials (MG-REQ-0254), so consumers never repeat it.
3. **Readable until (0.5):** each manifest carries `readable_until`, never shortened later
   (MG-REQ-0255).
4. **Withdrawn (0.7, with rollback):** a marker file per withdrawn manifest, written before the
   pointer moves; manifests stay immutable (MG-REQ-0256).
5. **Error codes (0.5):** two `MG-02xx` codes, expired and missing version,, for Mergus's own
   pinned reads (MG-REQ-0252.1). Consumers reading storage directly check `readable_until`.
6. **Credentials and encryption:** consumers hold their own read-only credentials (identities,
   0.6); no Mergus document carries a credential. Gold is encrypted only with `encrypted =
   true`; the entry says `encrypted`, and the key comes from the consumer's secret store.
7. **Retention (0.5):** manifests and catalog documents are never deleted (MG-REQ-0257); only
   data versions expire after the rollback window.
8. **Split gold:** not in the MVP. One environment's gold has one location; `connect` and
   `live` tables already live elsewhere and are described per entry. Because the location is
   per entry, a gold spread over several places later (with the Iceberg and Delta adapters,
   1.2) needs no manifest change.
