---
id: storage-zones
title: Storage zones and the single door to gold
crate: mergus-runtime
phase: "0.3"
status: draft
depends_on: [architecture-boundaries]
---

# Zones

| Zone | Holds | Lifetime | Writes | Reads |
|---|---|---|---|---|
| raw | Extracted data as received | Retention (default 30 days of loads) | Extraction | Models |
| work | Intermediate results of one run, spill files | Deleted when the run ends | Engines | Engines |
| ops | Ops tables and samples of failing rows | Two-tier retention (`ops/`) | `mergus-ops` | Per permissions |
| gold | Published tables and summaries | Kept while a manifest or the rollback window needs them | Publication only | Executor (pinned), BI |

### MG-REQ-0501 · Separate stores

raw and gold MUST be separate stores, each with its own credentials and retention. A profile
that would place both behind the same credentials MUST be a validation error:

1. MG-REQ-0501.1: with `local` or `shared`, the `location` of raw and of gold MUST differ, and
   neither may lie inside the other;
2. MG-REQ-0501.2: with `cloud`, the `catalog` secret references of raw and of gold MUST
   differ, and so MUST their `location` prefixes;
3. MG-REQ-0501.3: the error names both zones and the shared value, with the next free
   `MG-05xx` code.

- Kind: behavior
- Verify: test `mg_req_0501_raw_and_gold_separate_credentials`
- Decision: 0042

### MG-REQ-0502 · One direction

Data MUST flow only raw → work → gold. No step reads gold to write raw, and only the
publication step writes gold (INV-02).

- Kind: behavior
- Verify: test `mg_req_0502_data_flows_one_way`
- Phase: 0.5
- Depends: MG-REQ-0141

### MG-REQ-0503 · Work is ephemeral

The work zone of a run MUST be deleted when the run ends, successfully or not, and at the
start of the next run if a crash left it behind.

- Kind: behavior
- Verify: test `mg_req_0503_work_zone_cleaned`
- Phase: 0.4
- Decision: 0042

### MG-REQ-0504 · Spill stays local

Spill and temporary files MUST be written only to the configured local `temp_dir`, never to
network storage, and MUST respect `temp_max`.

- Kind: behavior
- Verify: test `mg_req_0504_spill_local_and_bounded`
- Phase: 0.9
- Decision: 0042
