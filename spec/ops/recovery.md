---
id: ops-recovery
title: Recovery ladder, backups and objectives
crate: mergus-runtime
phase: "0.9"
status: draft
depends_on: [contracts-manifest, environments-promotion]
---

# Recovery

| Problem | Remedy | Time |
|---|---|---|
| Something bad was published | `mergus rollback` | Seconds |
| One table went wrong days ago | `mergus restore-table <table> --at <time>` | Minutes |
| Catalog or storage lost or corrupted | `mergus restore --to <backup>` | By size |

### MG-REQ-1521 · Consistent backup

`mergus backup` MUST pause the writer briefly, record each environment's current manifest,
back up the DuckLake catalog, reference immutable data files (incremental copy), and back up
the Mergus catalog and journal, writing a backup manifest with hashes. Catalog and data MUST
come from the same point.

- Kind: behavior
- Verify: test `mg_req_1521_backup_is_consistent`

### MG-REQ-1522 · Maintenance never races backups

Compaction, expiry and cleanup MUST NOT run during a backup and MUST NOT delete files a
valid backup needs.

- Kind: behavior
- Verify: test `mg_req_1522_maintenance_respects_backups`

### MG-REQ-1523 · Verified backups

`mergus backup verify` MUST restore into a temporary space, run `audit verify` and compare
fingerprints, and MAY be scheduled.

- Kind: behavior
- Verify: test `mg_req_1523_backup_verify_restores_and_checks`

### MG-REQ-1524 · restore-table

`mergus restore-table <table> --at <time>` MUST publish the table's version valid at that
time, within storage retention, through the normal publication sequence.

- Kind: behavior
- Verify: test `mg_req_1524_restore_table_publishes_old_version`

### MG-REQ-1525 · Objectives

With `[recovery] rpo` and `rto` set, Mergus MUST schedule backups to meet the RPO and raise
an incident when the last verified restore took longer than the RTO.

- Kind: behavior
- Verify: test `mg_req_1525_rpo_rto_enforced`

### MG-REQ-1526 · 3-2-1 and encryption

`mergus doctor` MUST check that one backup copy is off the primary site and one is immutable;
backups MUST be encrypted.

- Kind: behavior
- Verify: test `mg_req_1526_backup_policy_checked`

### MG-REQ-1527 · Runbook

`mergus backup runbook` MUST generate a step-by-step recovery guide for the specific project.

- Kind: behavior
- Verify: test `mg_req_1527_runbook_generated`
