---
id: contracts-consumers
title: What host and executor need from Mergus (informative)
crate: none
phase: "0.5"
status: draft
depends_on: [contracts-manifest]
---

# Consumers

Informative. The host and executor specifications do not exist yet; this file states what
they rely on, so that Mergus keeps providing it. Mergus's obligations are the rules of
`contracts/manifest.md`.

| Consumer | Does | Relies on |
|---|---|---|
| Host | Reads `catalog/envs/<env>/latest`; if `schema_document_id` equals the loaded schema calls Menura `with_state`, else `load` with schema and state in one call | Manifest fields, catalog documents |
| Executor | Runs each Menura plan in a pinned session where each logical `source` resolves to the physical table and version of the manifest, without changing SQL or parameters | Pin recipe per table (MG-REQ-0249) |
| Executor credentials | Read-only on gold and on `connect`/`live` sources; catalog name equal to the `source` prefix | Identities (`security/identities.md`) |
| Host as UI host | Owns frame, sign-in, search, notifications, theme; mounts module pieces from their UI manifests | `ui/integration.md` |
| Distribution | Compatibility manifest of versions that work together and levels each executor supports | Manifest `mergus_version`, levels |

| Storage | Pinned read used by the executor |
|---|---|
| DuckLake | `ATTACH` at the snapshot, read-only |
| Iceberg | Per-table views `AT (VERSION => id)` |
| Delta | Version per table when attaching |
| Parquet | Views over file lists |
