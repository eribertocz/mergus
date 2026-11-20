---
id: distribution-install
title: Installer, version manager, on-demand adapters, offline and updates
crate: mergus-fetch
phase: "0.9"
status: draft
depends_on: [distribution-channels]
---

# Install and update

### MG-REQ-1621 · Verify before installing

The installer and every download by `mergus-fetch` MUST verify the Sigstore signature and the
checksum before installing; on mismatch they MUST abort and change nothing.

- Kind: behavior
- Verify: test `mg_req_1621_bad_signature_aborts`

### MG-REQ-1622 · No sudo, one folder

The installer MUST install into one user folder without administrator rights;
`mergus self uninstall` MUST remove everything it installed.

- Kind: behavior
- Verify: test `mg_req_1622_user_install_and_clean_uninstall`

### MG-REQ-1623 · Project version manager

When `project.toml` pins `mergus` and the running binary is outside the range, Mergus MUST
download, verify and run a matching version (MG-REQ-0319).

- Kind: behavior
- Verify: test `mg_req_1623_pinned_version_fetched_and_used`

### MG-REQ-1624 · Adapters on demand

Adapters and drivers MUST be downloaded on first use, verified and cached, keeping the core
binary small.

- Kind: behavior
- Verify: test `mg_req_1624_adapter_fetched_on_first_use`

### MG-REQ-1625 · Mirrors and offline

`MERGUS_MIRROR` MUST redirect every download to an internal mirror. `mergus bundle` MUST
build an offline package (binary, adapters, signatures) installable without network.
`--offline` MUST forbid every download.

- Kind: behavior
- Verify: test `mg_req_1625_mirror_bundle_offline`

### MG-REQ-1626 · Updates do not fight

`mergus self update` MUST exist only for installer-based installs; under other package
managers Mergus MUST print that manager's update command. Mergus MUST NOT check for updates
by itself.

- Kind: behavior
- Verify: test `mg_req_1626_self_update_only_for_installer`

### MG-REQ-1627 · self verify

`mergus self verify` MUST check the signature of the installed binary.

- Kind: behavior
- Verify: test `mg_req_1627_self_verify`

### MG-REQ-1628 · Python only when needed

A project with `.py` models MUST get its Python and Polars environment created and pinned by
Mergus itself; a SQL-only project MUST NOT require Python.

- Kind: behavior
- Verify: test `mg_req_1628_python_managed_only_when_needed`
