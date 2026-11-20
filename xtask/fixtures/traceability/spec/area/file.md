---
id: area-file
title: More fixture rules
crate: none
phase: "0.2" # a comment
status: draft
depends_on: []
---

# Area

### MG-REQ-9004 · Behavior rule verified by a check

- Kind: behavior
- Verify: check `something`

### MG-REQ-9005 · Behavior rule whose test fails

- Kind: behavior
- Verify: test `mg_req_9005_fails`

### MG-REQ-9006 · Process rule verified by a test

- Kind: process
- Verify: test `mg_req_9006_is_process`
