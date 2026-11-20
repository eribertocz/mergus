---
id: kernel
title: Fixture rules
crate: none
phase: "0.1"
status: draft
depends_on: []
---

# Kernel

### MG-REQ-9001 · Behavior rule with a passing test

- Kind: behavior
- Verify: test `mg_req_9001_passes`

### MG-REQ-9002 · Behavior rule without a test

- Kind: behavior
- Verify: test `mg_req_9002_is_missing`

### MG-REQ-9003 · Behavior rule of a later phase

- Kind: behavior
- Verify: test `mg_req_9003_is_later`
- Phase: 0.3

## A heading that ends the rule above

```markdown
### MG-REQ-9008 · A rule inside a code fence is not a rule

- Kind: behavior
- Verify: test `mg_req_9008_is_fenced`
```
