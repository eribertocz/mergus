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
