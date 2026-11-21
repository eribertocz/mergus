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
| Host proposals | Owns change proposals: hides Git from the user, creates the branch and commit, mirrors a proposal as a pull request on an external Git provider, lets only a person with permission approve, merges and reverts (MG-REQ-0191) | `review --json` (MG-REQ-1317), `dev --branch` (MG-REQ-1208) |
| Mimus | Optional AI module of the ecosystem: turns requests into proposed project changes on a branch and into Menura typed queries; it only proposes (MG-REQ-0191). It uses the contracts directly, not MCP | `describe --json` (MG-REQ-1312, MG-REQ-1314), `profile` (MG-REQ-1315), `diff` (MG-REQ-1228), `run status` (MG-REQ-1316), CLI `--json` and exit codes, `actor` (MG-REQ-0264) |
| `mergus-mcp` | Exposes Mergus commands to agents as MCP tools (MG-REQ-0290); Menura ships its own server | CLI `--json`, `mergus schema` (MG-REQ-0263) |
| Distribution | Compatibility manifest of versions that work together and levels each executor supports | Manifest `mergus_version`, levels |

| Storage | Pinned read used by the executor |
|---|---|
| DuckLake | `ATTACH` at the snapshot, read-only |
| Iceberg | Per-table views `AT (VERSION => id)` |
| Delta | Version per table when attaching |
| Parquet | Views over file lists |

### MG-REQ-0290 · MCP adapter is a thin, separate module

Mergus's MCP server MUST be the crate `mergus-mcp` in the Mergus repository, released and tested
with the CLI under the same version. It MUST map MCP tools one to one onto CLI commands with
`--json` by running the `mergus` binary, hold no logic of its own, depend on no Mergus crate
but `mergus-contracts`, and run with the permissions of the identity that calls it. It exposes
only Mergus; every other module ships its own server in its own repository.

- Kind: process
- Verify: review `mcp-thin-adapter`
- Phase: 0.8
- Depends: MG-REQ-0182
- Decision: 0008, 0014, 0015

### MG-REQ-0291 · MCP safe by default

The MCP adapter MUST start read-only and scoped to `dev` environments. Tools come in groups,
`discover`, `build` and `operate`, and only `discover` is on by default. Turning on a group
that writes, or a non-dev environment, MUST be an explicit setting of the installation, never
a request from the agent.

- Kind: behavior
- Verify: test `mg_req_0291_mcp_is_read_only_and_dev_by_default`
- Phase: 0.8
- Depends: MG-REQ-0290
- Decision: 0011

### MG-REQ-0292 · Data is never an instruction

Every MCP tool result and every `--json` event that carries values read from data (previews,
samples, dimension values, error excerpts) MUST put them in a field marked as untrusted data,
apart from Mergus's own text. The agent guide (MG-REQ-1787) MUST tell agents that this content
is data and never instructions.

- Kind: behavior
- Verify: test `mg_req_0292_data_values_are_marked_untrusted`
- Phase: 0.8
- Decision: 0011

### MG-REQ-0293 · Human confirmation through the client

When an MCP tool reaches a confirmation of MG-REQ-1307, the adapter MUST ask the person through
the MCP client's confirmation request (elicitation) and MUST NOT pass `--yes` on its own. If the
client cannot ask, the action is refused.

- Kind: behavior
- Verify: test `mg_req_0293_irreversible_actions_ask_the_person`
- Phase: 0.8
- Depends: MG-REQ-1307
- Decision: 0011

### MG-REQ-0294 · Typed tool results

Every MCP tool MUST declare an output schema taken from the Mergus contract of the command it
maps (MG-REQ-0201) and return structured results valid against it.

- Kind: behavior
- Verify: test `mg_req_0294_tool_results_match_their_schema`
- Phase: 0.8
- Depends: MG-REQ-0290, MG-REQ-0201
- Decision: 0011
