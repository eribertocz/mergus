---
id: ops-upgrades
title: State migrations, upgrade checks and deprecation
crate: mergus-runtime
phase: "0.9"
status: draft
depends_on: [ops-recovery]
---

# Upgrades

### MG-REQ-1541 · State version is separate

The state format MUST have its own version. A binary newer than the state MUST require
`mergus migrate`, which backs up first and supports `rollback`; a binary older than the state
MUST refuse to run with a clear message.

- Kind: behavior
- Verify: test `mg_req_1541_state_version_checked`

### MG-REQ-1542 · Upgrading does not recompute everything

Fingerprints MUST NOT include the Mergus version, only content and calculation rules, so
that upgrading does not rebuild unchanged models.

- Kind: behavior
- Verify: test `mg_req_1542_upgrade_keeps_fingerprints`

### MG-REQ-1543 · upgrade --check

`mergus upgrade --check` MUST run the new version on a sample and compare results with prod,
reporting how many models change.

- Kind: behavior
- Verify: test `mg_req_1543_upgrade_check_compares`

### MG-REQ-1544 · Blue/green upgrade

`mergus upgrade --env <name>` MUST build with the new version in a separate environment that
is then compared and promoted like any change; prod never stops.

- Kind: behavior
- Verify: test `mg_req_1544_blue_green_upgrade`

### MG-REQ-1545 · Pinned engines

Each Mergus version MUST pin exact DuckDB and Polars versions; changing them is a new Mergus
version with behavior-change notes.

- Kind: process
- Verify: check `pinned-engines`

### MG-REQ-1546 · Deprecation policy

A deprecated option MUST keep working with a warning for at least two minor versions and six
months, with a suggested migration applied by `mergus migrate --project`.

- Kind: process
- Verify: review `deprecation-policy`
