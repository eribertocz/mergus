---
id: spec-conventions
title: How this specification is written and read
crate: none
phase: "0.1"
status: draft
depends_on: []
---

# How this specification is written and read

This file is for the **specification author**. Implementation sessions do not read it: they
receive their context from `just spec-context` (section 6).

## 1. What Mergus is, in one paragraph

Mergus is an ELT tool written in Rust. It extracts data from databases and APIs into an
encrypted raw layer, transforms it with DuckDB SQL and Polars, tests it, and publishes gold
tables plus a catalog that the semantic compiler (Menura) and BI tools consume. It speaks to
other modules only through public contracts.

## 2. Normative language

- The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are used as described in RFC 2119
  and RFC 8174, and only when written in capitals.
- Text outside a rule block is explanatory and never normative.
- Examples are normative only when a rule says so.

## 3. Reading levels

| Level | Files | Who reads them |
|---|---|---|
| Always | `kernel.md` | Every session, at start |
| Per task | The output of `just spec-context` | Implementer and reviewer, for their issue |
| Per area | `MAP.md`, then the files of one area | Specification author |
| Never for implementing | `decisions/`, `roadmap.md`, `status.md` | Specification author and owner only |

A session that needs a file outside its level stops and opens a `spec-defect` issue saying
which information is missing from its context. Missing context is a defect of the issue, not
a reason to read more.

## 4. File rules

### 4.1 Layout

```
spec/
  README.md  kernel.md  MAP.md  glossary.md  roadmap.md
  RULES.md  crates.md  status.md        generated, never edited by hand
  <area>/<topic>.md                     one topic per file
  decisions/NNNN-<title>.md             architecture decision records
```

Areas: `architecture`, `contracts`, `project`, `extract`, `storage`, `model`, `run`,
`quality`, `publish`, `security`, `observability`, `environments`, `cli`, `ui`, `ops`,
`distribution`, `governance`.

### 4.2 Header

Every specification file except generated files and decision records starts with this YAML
header. `spec_tool lint` rejects a file whose header is missing a field or has an unknown one.

```yaml
---
id: extract-rest            # unique, kebab-case, equals <area>-<file stem>
title: REST API sources     # one line
crate: mergus-extract-rest  # crate that implements it, or "none"
phase: "0.4"                # roadmap phase in which its rules are due
status: draft               # draft | stable | retired
depends_on: [contracts-adapter, MG-REQ-0101]   # file ids or rule ids
---
```

### 4.3 Size and scope

- One file describes one component or one concern. If its description needs "and", split it.
- A file has at most 300 lines.
- A rule lives in exactly one file. Other files cite it by ID and never restate it.

## 5. Rule format

### 5.1 Shape

Every rule is a level-3 heading followed by its statement and a metadata list:

```markdown
### MG-REQ-0412 · Refresh OAuth2 tokens before expiry

The REST adapter MUST refresh an OAuth2 access token when less than 60 seconds of its
lifetime remain, before sending the next request.

- Kind: behavior
- Verify: test `mg_req_0412_token_refreshed_before_expiry`
- Phase: 0.4
- Decision: 0007
```

| Field | Required | Values |
|---|---|---|
| `Kind` | yes | `behavior` (observable from outside the crate) or `process` (how we work) |
| `Verify` | yes | `test <name>`, `check <ci-job>` or `review <checklist-item>` |
| `Phase` | no | Overrides the file's phase for this rule |
| `Decision` | no | Number of the decision record that explains it |
| `Depends` | no | Other rule IDs this rule needs |

- The statement is one paragraph, or a short numbered list whose items are `MG-REQ-0412.1`,
  `MG-REQ-0412.2` and so on.
- Every `behavior` rule is verified by at least one test whose name starts with its ID in
  snake case (`mg_req_0412_`). Every `process` rule is verified by a CI check or a review item.

### 5.2 Identifiers

- IDs are never reused and never renumbered. A rule that no longer applies keeps its heading,
  gets `- Status: retired` and a one-line reason.
- Each area owns a range. The next free ID is the highest used in the range plus one.

| Range | Area | Range | Area |
|---|---|---|---|
| 0001–0099 | kernel | 0900–0999 | publish |
| 0100–0199 | architecture | 1000–1099 | security |
| 0200–0299 | contracts | 1100–1199 | observability |
| 0300–0399 | project | 1200–1299 | environments |
| 0400–0499 | extract | 1300–1399 | cli |
| 0500–0599 | storage | 1400–1499 | ui |
| 0600–0699 | model | 1500–1599 | ops |
| 0700–0799 | run | 1600–1699 | distribution |
| 0800–0899 | quality | 1700–1799 | governance |

### 5.3 Other identifiers

| Kind | Form | Example | Defined in |
|---|---|---|---|
| Error code | `MG-` + 4 digits | `MG-1042` | `cli/errors.md` |
| Invariant | `INV-` + 2 digits | `INV-03` | `architecture/invariants.md` |
| Decision record | 4 digits | `0007` | `decisions/` |

## 6. Tooling

`scripts/spec_tool.py` (stdlib Python, no dependencies) is the only way to produce the
generated files and the per-task context. `just` recipes wrap it.

| Command | Does |
|---|---|
| `just spec-lint` | Checks headers, sizes, rule shape, unique IDs, ID ranges and links |
| `just spec-index` | Writes `RULES.md` and `crates.md` |
| `just spec-status` | Writes `status.md` from the rules and the test names found in the code |
| `just spec-context <issue \| IDs>` | Prints the kernel, the explanatory text (tables, diagrams) of the files of the rules asked for, those rules and their direct dependencies |
| `just spec-issue <IDs>` | Prints an issue body ready to file |
| `just spec-check` | Fails if any generated file is out of date (CI runs this) |

## 7. Changing the specification

1. Only a specification session changes `spec/`, on a branch `spec/<topic>`.
2. A change to the meaning of an existing rule needs a decision record in `decisions/`.
3. The pull request runs `just spec-lint` and `just spec-check`.
4. The owner merges.
