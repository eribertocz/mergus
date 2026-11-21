---
id: governance-docs
title: User documentation
crate: none
phase: "0.8"
status: draft
depends_on: [cli-errors]
---

# Documentation

### MG-REQ-1781 · Four kinds of page

User documentation in `docs/` MUST be organized as tutorials, how-to guides, reference and
explanations (Diátaxis), in English. Translations come after the MVP.

- Kind: process
- Verify: review `docs-structure`

### MG-REQ-1782 · Generated reference

The reference of commands, of every TOML key and of every contract MUST be generated from the
CLI parser and the JSON Schemas; CI fails when it is out of date.

- Kind: process
- Verify: check `docs-generated`

### MG-REQ-1783 · Examples that run

Every code example in the documentation MUST run in CI against the simulator.

- Kind: process
- Verify: check `docs-examples`

### MG-REQ-1784 · Offline docs

`mergus docs` MUST serve the documentation of the installed version without network.

- Kind: behavior
- Verify: test `mg_req_1784_docs_offline`

### MG-REQ-1785 · Docs for AI assistants

The documentation site MUST publish `llms.txt` and clean Markdown pages.

- Kind: process
- Verify: check `docs-llms`

### MG-REQ-1786 · One vocabulary

CLI messages, UI and documentation MUST use the terms of `spec/glossary.md`; a vocabulary
linter in CI flags forbidden synonyms.

- Kind: process
- Verify: check `vocabulary`

### MG-REQ-1787 · Agent guide

The documentation MUST include a page "Use Mergus from an agent", linked from `llms.txt`,
generated from `mergus schema --all` and `describe --json`. It shows the approval-by-Git flow
(MG-REQ-0191), the exit codes, `hint` and `next` (MG-REQ-1334) and the MCP adapter
(MG-REQ-0290), with one example that runs in CI.

- Kind: process
- Verify: check `docs-agent-guide`
- Depends: MG-REQ-1785, MG-REQ-0191
- Decision: 0010
