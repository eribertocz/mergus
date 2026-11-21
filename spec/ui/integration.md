---
id: ui-integration
title: UI architecture, ecosystem rule, two modes and security
crate: none
phase: "1.1"
status: draft
depends_on: [contracts-overview]
---

# UI integration

The UI answers "are my data OK?" in three seconds and leads to cause and action. It observes
and operates; it never edits code (Git is the source of truth). It comes after the MVP.

Ecosystem rule, agreed with Menura: **the host is the only owner of the experience. Modules
contribute isolated pieces declared in a manifest and data through contracts, share a design
published as W3C design tokens, reference objects by stable identifiers, never import
another module's code or read its internal data, and never show what the user cannot see.**

| | Standalone (`mergus ui`) | Integrated (inside the host) |
|---|---|---|
| Frame | Mergus mini-host | The host's |
| Style | Mergus tokens | Host tokens |
| Sign-in | Local token link; OIDC in server mode | The host's; Mergus never authenticates |

### MG-REQ-1401 · Contracts only

The UI MUST consume only public contracts: the Mergus HTTP API (OpenAPI), the event stream
(Server-Sent Events) and Arrow IPC for data. It MUST contain no business logic and keep no
state of its own; views read the ops tables.

- Kind: behavior
- Verify: review `ui-contracts-only`

### MG-REQ-1402 · Same pieces in both modes

The mini-host MUST implement the same UI manifest and message protocol as the ecosystem
host, so that every piece works unchanged in both modes. Pieces MUST use only semantic
design tokens, never fixed colors, fonts or sizes.

- Kind: behavior
- Verify: test `mg_req_1402_pieces_mount_in_reference_host`

### MG-REQ-1403 · Isolated, validated, lazy pieces

Each piece MUST run isolated (Web Component with Shadow DOM or iframe), validate every
message against its schema and origin, load lazily, declare its required permissions and the
minimum host version, and meet size and first-paint budgets checked in CI.

- Kind: behavior
- Verify: test `mg_req_1403_piece_isolation_and_budgets`

### MG-REQ-1404 · What cannot be seen does not appear

The UI MUST NOT show objects, links or counts the user is not authorized for; every link is
re-authorized at its destination; Menura's compiled SQL is never shown to end users; `pii`
columns are masked unless explicitly permitted.

- Kind: behavior
- Verify: test `mg_req_1404_unauthorized_objects_absent`

### MG-REQ-1405 · Actions go through the runtime

Every UI action that changes something MUST go through the same runtime as the CLI, with
confirmation, an idempotency key and audit, and MUST show its equivalent CLI command.

- Kind: behavior
- Verify: test `mg_req_1405_ui_action_equals_cli`

### MG-REQ-1406 · Self-contained and private

The UI MUST load no third-party scripts, fonts or CDNs, use a strict content security policy,
send no telemetry and work without internet.

- Kind: behavior
- Verify: test `mg_req_1406_ui_offline_and_strict_csp`

### MG-REQ-1407 · Trusted ecosystem pieces

The mini-host MUST mount a piece of another module (for example the shared AI chat) only when
the host's or the installation's UI manifest declares it, with its origin and integrity hash;
it MUST never load an undeclared piece. Declared pieces follow MG-REQ-1403 to MG-REQ-1405; this
is the only exception to "no third-party scripts" of MG-REQ-1406.

- Kind: behavior
- Verify: test `mg_req_1407_only_declared_pieces_mount`
- Depends: MG-REQ-1402, MG-REQ-1406
- Decision: 0008
