---
id: cli-errors
title: Error code catalog
crate: mergus-cli
phase: "0.2"
status: draft
depends_on: [cli-output]
---

# Error codes

Error codes are `MG-` plus four digits. The first two digits name the area, mirroring the rule
ranges of `spec/README.md` (for example `MG-04xx` extraction, `MG-10xx` security). The
catalog lives in `crates/mergus-cli/errors/MG-####.md`, one file per code:

```markdown
# MG-0301 · Two models have the same name

- Exit code: 1
- Retryable: no
- Rules: MG-REQ-0303

## Cause
One sentence or more.

## Example
What triggers it.

## Fix
What to change, with an example.
```

A retired code adds the line `- Status: retired` to the list.

| Range | Area | Range | Area |
|---|---|---|---|
| MG-01xx | architecture, internal | MG-09xx | publish |
| MG-02xx | contracts | MG-10xx | security |
| MG-03xx | project | MG-11xx | observability |
| MG-04xx | extract | MG-12xx | environments |
| MG-05xx | storage | MG-13xx | cli |
| MG-06xx | model | MG-15xx | ops |
| MG-07xx | run | MG-16xx | distribution |
| MG-08xx | quality | MG-14xx | ui |
| | | MG-17xx | governance |

### MG-REQ-1331 · Every code is documented

Each code MUST have a file with: title, exit code, retryable or not, cause, example, fix, and
the rule IDs it enforces. CI MUST fail when code emits an undocumented code or a documented
code is never emitted by any test.

- Kind: process
- Verify: check `error-catalog`

### MG-REQ-1332 · Codes are stable

An error code MUST never change meaning or be reused. A retired code keeps its file with a
`retired` note.

- Kind: process
- Verify: check `error-catalog`

### MG-REQ-1333 · Code range matches area

A code MUST be in the range of the area of the rule it enforces.

- Kind: process
- Verify: check `error-catalog`

### MG-REQ-1334 · Actionable errors

Every `diagnostic` event MUST carry `hint`, one sentence that says what to do next, and MAY
carry `next`, a list of CLI invocations (each a list of arguments) that address it, such as
`["mergus", "explain", "sales"]`. `hint` and `next` MUST come from deterministic templates,
MUST NOT contain secrets or data values, and are never run automatically.

- Kind: behavior
- Verify: test `mg_req_1334_diagnostics_are_actionable`
- Decision: 0010

### MG-REQ-1335 · Error catalog check

The `error-catalog` check of `cargo xtask ci` MUST fail when:

1. MG-REQ-1335.1: a file under `crates/mergus-cli/errors/` is not named `MG-<dddd>.md`, or its
   first line is not `# MG-<dddd> · <title>` with the same code and a non-empty title;
2. MG-REQ-1335.2: a file lacks the list lines `- Exit code: <n>`, `<n>` being a code of the
   table of `contracts/events.md` section 2, `- Retryable: yes` or `- Retryable: no`, and
   `- Rules: ` followed by rule IDs separated by `, `, each present in `spec/RULES.md`; or lacks
   one of the headings `## Cause`, `## Example` and `## Fix`, or has one with no text;
3. MG-REQ-1335.3: the first two digits of a code differ from the first two digits of a rule
   it lists, except that a code listing kernel rules (`MG-REQ-00xx`) only is in `MG-01xx`;
4. MG-REQ-1335.4: a code `MG-<dddd>` written in a `.rs` file under `crates/` outside test
   sources has no file, or its file has `- Status: retired`;
5. MG-REQ-1335.5: the code of a file without `- Status: retired` is written in no test
   source. Test sources are the files under a `tests/` directory, the files named `tests.rs`,
   and, in any other `.rs` file, the lines from its first `#[cfg(test)]` line to its end;
6. MG-REQ-1335.6: the codes of an area, the first two digits, do not run from `<aa>01` to
   their highest without a gap, so a code is never removed or reused (MG-REQ-1332).

A code takes the area of the rule that defines its error. "The next free code" is the next one
on `main` when the pull request merges `main`: when two open pull requests take the same code,
the one that merges second renumbers its codes. `MG-REQ-` identifiers are not codes. With no catalog directory, only item 4 applies.

- Kind: process
- Verify: check `error-catalog`
- Depends: MG-REQ-1331, MG-REQ-1332, MG-REQ-1333
- Decision: 0017
