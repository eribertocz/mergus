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
| Shared AI module | Translates natural language into proposed project changes on a branch and into Menura typed queries; it only proposes (MG-REQ-0191) | `describe --json` (MG-REQ-1312), CLI `--json` and exit codes, `actor` (MG-REQ-0264) |
| MCP adapter | Exposes Mergus and Menura commands to agents as MCP tools (MG-REQ-0290) | CLI `--json`, `mergus schema` (MG-REQ-0263) |
| Distribution | Compatibility manifest of versions that work together and levels each executor supports | Manifest `mergus_version`, levels |

| Storage | Pinned read used by the executor |
|---|---|
| DuckLake | `ATTACH` at the snapshot, read-only |
| Iceberg | Per-table views `AT (VERSION => id)` |
| Delta | Version per table when attaching |
| Parquet | Views over file lists |

### MG-REQ-0290 · MCP adapter is a thin, separate module

The MCP adapter MUST be a module separate from Mergus that maps MCP tools one to one onto CLI
commands with `--json`, holds no logic of its own, and runs with the permissions of the
identity that calls it.

- Kind: process
- Verify: review `mcp-thin-adapter`
- Phase: 0.8
- Depends: MG-REQ-0182
- Decision: 0008
