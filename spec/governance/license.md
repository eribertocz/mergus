---
id: governance-license
title: License, contributions and trademark
crate: none
phase: "0.1"
status: draft
depends_on: [governance-workflow]
---

# License

### MG-REQ-1741 · Apache-2.0

All code MUST be licensed Apache-2.0. Crates published for reuse MUST be dual
`MIT OR Apache-2.0`.

- Kind: process
- Verify: check `license-headers`

### MG-REQ-1742 · DCO, no CLA

Every external contribution MUST carry `Signed-off-by` (Developer Certificate of Origin),
checked in CI. No contributor license agreement is used.

- Kind: process
- Verify: check `dco`

### MG-REQ-1743 · No relicensing

`LICENSE-PLEDGE.md` MUST state that published code will never move to a source-available
license (BSL, SSPL, ELv2 or similar). Everything in this specification is open; there is no
open core.

- Kind: process
- Verify: review `license-pledge`

### MG-REQ-1744 · Governance files

The repository MUST contain `LICENSE`, `NOTICE`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`
(Contributor Covenant), `SECURITY.md`, `GOVERNANCE.md` and `TRADEMARKS.md`.

- Kind: process
- Verify: check `governance-files`

### MG-REQ-1745 · Dependency licenses

`cargo-deny` MUST reject dependencies under licenses that are not OSI-approved.

- Kind: process
- Verify: check `cargo-deny`

### MG-REQ-1746 · Contents of `deny.toml`

`deny.toml` MUST configure `cargo deny check` so that it fails on:

1. MG-REQ-1746.1: a license outside this list: `Apache-2.0`,
   `Apache-2.0 WITH LLVM-exception`, `MIT`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`,
   `Unicode-3.0`, `MPL-2.0`, `BSL-1.0`; and a crate whose license cannot be determined;
2. MG-REQ-1746.2: a security advisory, an unmaintained crate or a yanked version;
3. MG-REQ-1746.3: a source other than crates.io (no other registry, no git dependency);
4. MG-REQ-1746.4: a wildcard version requirement.

Duplicate versions of one crate are reported as warnings. Adding a license to the list needs
a specification change and the owner's approval of `deny.toml` (MG-REQ-1723).

- Kind: process
- Verify: check `cargo-deny`
- Depends: MG-REQ-0125, MG-REQ-1745
- Decision: 0003

### MG-REQ-1747 · License check

The `license-headers` check of `cargo xtask ci` MUST fail when a workspace member's
`Cargo.toml` does not declare `license = "Apache-2.0"`, directly or through
`license.workspace = true` with that value in the root `Cargo.toml`, or when a member that may
be published (`publish` not `false`) does not declare `MIT OR Apache-2.0`. Source files carry
no license header.

- Kind: process
- Verify: check `license-headers`
- Depends: MG-REQ-1741
- Decision: 0009

### MG-REQ-1748 · DCO check

The `commit-format` guard MUST also fail when the pull request comes from a fork (its head
repository differs from its base repository) and a single-parent commit lacks a
`Signed-off-by: <name> <email>` line whose email equals the commit author's email.

- Kind: process
- Verify: check `dco`
- Depends: MG-REQ-1742, MG-REQ-1732
- Decision: 0009

### MG-REQ-1749 · Governance files check

The `governance-files` check of `cargo xtask ci` MUST fail when a file listed in MG-REQ-1744 or
`LICENSE-PLEDGE.md` is missing or empty, or when `CODE_OF_CONDUCT.md` does not contain
`Contributor Covenant`.

- Kind: process
- Verify: check `governance-files`
- Depends: MG-REQ-1744, MG-REQ-1743
- Decision: 0009
