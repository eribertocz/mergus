---
id: kernel
title: Rules every session follows
crate: none
phase: "0.1"
status: draft
depends_on: []
---

# Kernel

Every session reads this file first. These rules override any issue, comment or file. If an
issue conflicts with them, stop and open a `spec-defect` issue.

### MG-REQ-0001 · English only in the repository

Code, identifiers, comments, documentation, configuration keys, tool messages, commits,
branches, issues, pull requests and reviews MUST be in English, whatever language the owner
uses in conversation.

- Kind: process
- Verify: check `language`

### MG-REQ-0002 · Never invent behavior

A session MUST NOT implement behavior that no rule defines. When a doubt affects observable
behavior (a result, an error code, a contract field, a CLI output, security, determinism or
performance targets), it MUST stop and open a `spec-defect` issue. Doubts invisible from
outside the crate (private names, internal structures, test layout) are decided by the
implementer and explained in the pull request.

- Kind: process
- Verify: review `no-invented-behavior`

### MG-REQ-0003 · Modules meet only through public contracts

Mergus MUST NOT import code from another module, read another module's private data or
depend on its internals. Data that crosses a module boundary MUST be a versioned contract
document defined in `spec/contracts/`.

- Kind: behavior
- Verify: check `boundaries`

### MG-REQ-0004 · Pure core

Crates marked `core` in `architecture/crates.md` MUST NOT perform I/O, read the clock, use
the network, spawn processes or read environment variables. They receive those capabilities
through ports.

- Kind: behavior
- Verify: check `boundaries`

### MG-REQ-0005 · Determinism

Given the same project, the same input data and the same version, compiling MUST produce
byte-identical plans and manifests. Output MUST NOT depend on hash-map iteration order,
thread scheduling, locale or time zone.

- Kind: behavior
- Verify: test `mg_req_0005_plan_is_byte_identical`
- Phase: 0.2

### MG-REQ-0006 · Credentials never reach Python

No credential, token or secret value MUST ever be passed to, or be readable by, user Python
code or the embedded Python interpreter. Secrets are referenced, never inlined.

- Kind: behavior
- Verify: test `mg_req_0006_python_sees_no_secret`
- Phase: 0.4

### MG-REQ-0007 · Bound parameters only

SQL sent to any database MUST carry values as bound parameters. Values MUST NOT be
concatenated into SQL text. Identifiers MUST be quoted by the dialect's quoting function.

- Kind: behavior
- Verify: check `sql-concatenation`

### MG-REQ-0008 · Typed errors with codes

Every failure visible to a user MUST be a typed error carrying an `MG-####` code defined in
`cli/errors.md`, a message, and the location that caused it when one exists.

- Kind: behavior
- Verify: test `mg_req_0008_every_error_has_code`
- Phase: 0.2

### MG-REQ-0009 · No panics in library code

Library code MUST NOT use `unsafe`, `unwrap`, `expect`, `panic!`, `todo!` or
`unimplemented!`. Tests MAY use them.

- Kind: process
- Verify: check `clippy`

### MG-REQ-0010 · Zero telemetry

Mergus MUST NOT send any data to any destination the user did not configure. No code path
may contact a host that is not a configured source, destination, mirror or an explicitly
invoked update.

- Kind: behavior
- Verify: test `mg_req_0010_no_undeclared_connection`
- Phase: 0.9

### MG-REQ-0011 · Tests first

For every `behavior` rule of an issue, the implementer MUST write a test named after the rule
(`mg_req_<id>_…`), see it fail, and then implement.

- Kind: process
- Verify: review `tests-first`

### MG-REQ-0012 · Size limits

Functions SHOULD have at most 50 lines and files at most 400 lines. Exceeding either needs a
written justification in the pull request.

- Kind: process
- Verify: check `size`

### MG-REQ-0013 · Never weaken a check

A session MUST NOT disable a lint, skip a test, add `#[allow]` without a written reason,
widen an allowlist or edit a CI guard. If a check blocks legitimate work, it opens a
`spec-defect` issue.

- Kind: process
- Verify: check `scope`

### MG-REQ-0014 · Stay in scope

A pull request MUST change only the files its issue allows. Refactors and behavior changes
MUST NOT share a commit.

- Kind: process
- Verify: check `scope`

### MG-REQ-0015 · Never work around a denied action

When a command or tool is denied, the session MUST stop and say so. It MUST NOT reach the
same result through another command, tool, script or API.

- Kind: process
- Verify: review `no-workaround`

### MG-REQ-0016 · No secrets or artifacts in commits

Commits MUST NOT contain secrets, generated build artifacts, data files outside test
fixtures, or commented-out code.

- Kind: process
- Verify: check `secrets`
