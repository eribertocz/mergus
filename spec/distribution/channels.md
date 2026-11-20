---
id: distribution-channels
title: Platforms, channels and releases
crate: none
phase: "0.9"
status: draft
depends_on: [security-protection]
---

# Channels

| Channel | MVP |
|---|---|
| Installer `install.sh` / `install.ps1` (version pinnable in the URL) | yes |
| GitHub Releases | yes |
| PyPI (`pip install mergus`, `uvx mergus`): maturin wheel with the binary and `mergus.testing` | yes |
| Homebrew (tap, then core) | yes |
| Docker: multi-arch distroless non-root image, plus a `-python` variant | yes |
| GitHub Action `setup-mergus` with cache | yes |
| WinGet, Scoop, apt/rpm, conda-forge, Nix flake, mise/aqua, devcontainer feature, Helm | after MVP |

### MG-REQ-1601 · Platforms

Every release MUST ship static binaries for Linux x86_64 and arm64 (musl), macOS arm64 and
x86_64, and Windows x64.

- Kind: process
- Verify: check `release-artifacts`

### MG-REQ-1602 · One artifact, many doors

Every channel MUST distribute the same signed binaries of the GitHub release, never a
separately built one.

- Kind: process
- Verify: check `release-artifacts`

### MG-REQ-1603 · Release in one gesture

A tag `v<semver>` MUST make `dist` publish every MVP channel, signed, with the generated
changelog.

- Kind: process
- Verify: check `release-trigger`

### MG-REQ-1604 · Release channels

Mergus MUST offer `stable`, `beta` and `nightly` channels.

- Kind: process
- Verify: check `release-channels`

### MG-REQ-1605 · Try without installing

`uvx mergus init --demo` and `docker run <image> init --demo` MUST work on a clean machine.

- Kind: behavior
- Verify: test `mg_req_1605_try_without_installing`
