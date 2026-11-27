---
id: architecture-code
title: Code rules beyond the kernel
crate: none
phase: "0.1"
status: draft
depends_on: [kernel]
---

# Code rules

The kernel already forbids panics, `unsafe` in general, untyped errors and oversized
functions. This file adds the rules specific to how crates are written.

### MG-REQ-0170 · Unsafe only in FFI crates

`#![forbid(unsafe_code)]` MUST be set in every crate except those binding foreign code
(DuckDB, Polars, Python). There, each `unsafe` block MUST carry a `// SAFETY:` comment and a
test.

- Kind: process
- Verify: check `unsafe`

### MG-REQ-0171 · Error types

Library crates MUST define errors with `thiserror`. `anyhow` MAY be used only in
`mergus-cli` and `xtask`. Invariant assertions are the only allowed panics, and they are
listed in `architecture/invariants.md`.

- Kind: process
- Verify: check `clippy`

### MG-REQ-0172 · Logging through ops

Logs and traces MUST use `tracing` and be configured only by `mergus-ops`. Crates MUST NOT
print to standard output or standard error, except `mergus-cli`.

- Kind: process
- Verify: check `clippy`

### MG-REQ-0173 · Names state intent

Names MUST be full English words that state intent. Abbreviations other than widely known
ones (`id`, `sql`, `url`, `io`) MUST NOT be used.

- Kind: process
- Verify: review `naming`

### MG-REQ-0189 · Portable paths

Code MUST handle paths with the standard library's path types and compare them by
components, never as strings joined with `/` or `\`. Every path a tool prints (findings,
reports, errors) MUST be relative to the workspace root when it lies inside it and MUST use
`/` as the separator on every operating system. Tests that touch paths MUST pass on Linux,
macOS and Windows.

- Kind: process
- Verify: review `portable-paths`
- Decision: 0004

### MG-REQ-0174 · Invalid states unrepresentable

Domain values MUST use enums and newtypes rather than strings and boolean flags when the set
of valid values is known.

- Kind: process
- Verify: review `types`

### MG-REQ-0175 · Public items documented

Every public item MUST have documentation stating what it does and its errors, with an
example where useful.

- Kind: process
- Verify: check `missing-docs`

## Content checks

The checks `language`, `size`, `secrets` and `sql-concatenation` read files, not the build.
These rules define what each one scans and reports, so that their result is not left to the
implementer (decision 0002).

### MG-REQ-0176 · Scope of the content checks

The content checks MUST scan the files tracked by git (`git ls-files`) and skip every path
that has a directory named `fixtures` and the generated files `spec/RULES.md`
and `spec/crates.md`. Each finding MUST be reported as
`<path>:<line>: <check>: <message>`, sorted by path and line.

- Kind: process
- Verify: review `content-check-scope`
- Decision: 0002

### MG-REQ-0177 · Language check

The `language` check MUST report every line of a tracked text file that contains an alphabetic
character outside ASCII. It is a warning: it MUST always exit with success.

- Kind: process
- Verify: check `language`
- Depends: MG-REQ-0176, MG-REQ-0001
- Decision: 0002

### MG-REQ-0178 · Size check

The `size` check MUST measure Rust files (`*.rs`) only, tests included, and fail on:

1. MG-REQ-0178.1: a file of more than 400 lines;
2. MG-REQ-0178.2: a function of more than 50 lines, counted from the line of `fn` to its
   closing brace, both included, blank and comment lines included.

An item is exempt when it carries the written justification MG-REQ-0012 asks for, as a
waiver comment `// size: <reason>` with a non-empty reason: in the first five lines of the
file for a file, or on the line directly above the function's attributes and documentation
for a function. A waiver is not an allowlist: the review checks each reason, and the pull
request repeats it.

- Kind: process
- Verify: check `size`
- Depends: MG-REQ-0176, MG-REQ-0012
- Decision: 0002

### MG-REQ-0179 · Secrets and artifacts check

The `secrets` check MUST fail on:

1. MG-REQ-0179.1: a line that matches `-----BEGIN [A-Z ]*PRIVATE KEY-----`,
   `AKIA[0-9A-Z]{16}`, `gh[pousr]_[A-Za-z0-9]{36,}`, `github_pat_[A-Za-z0-9_]{22,}` or
   `xox[abprs]-[A-Za-z0-9-]{10,}`;
2. MG-REQ-0179.2: a file named `.env` or `.env.<anything>`, or with extension `.pem`,
   `.key`, `.p12` or `.pfx`;
3. MG-REQ-0179.3: a build artifact: a path under `target/`, or a file with extension
   `.rlib`, `.rmeta`, `.so`, `.dylib`, `.dll`, `.exe`, `.o`, `.a`, `.pyc` or `.whl`;
4. MG-REQ-0179.4: a data file outside a `tests/` directory: extension `.parquet`, `.csv`,
   `.tsv`, `.arrow`, `.feather`, `.avro`, `.duckdb`, `.db`, `.sqlite` or `.xlsx`;
5. MG-REQ-0179.5: commented-out code in a Rust file: a comment line that starts with `//`
   but not `///` or `//!`, and whose text, trimmed, ends with `;`, `{` or `}`.

Secrets that match no pattern are left to review (MG-REQ-0016).

- Kind: process
- Verify: check `secrets`
- Depends: MG-REQ-0176, MG-REQ-0016
- Decision: 0002

### MG-REQ-0184 · SQL concatenation check

The `sql-concatenation` check MUST fail on a Rust string literal passed to `format!`,
`write!`, `writeln!` or `concat!` whose text, trimmed and compared without case, starts with
`SELECT`, `INSERT`, `UPDATE`, `DELETE`, `MERGE`, `CREATE`, `DROP`, `ALTER`, `WITH`, `COPY` or
`ATTACH` and contains a `{` placeholder. A literal is exempt when the line directly above it
is the waiver comment `// sql: identifiers quoted by <function>`, naming the dialect's
quoting function (MG-REQ-0007); the review checks that every placeholder is such an
identifier. No other way around the check is allowed: building the same text with `concat`,
`push_str` or `+` breaks this rule. Text that must be composed from data, such as a macro
definition sent to the parser, is joined inside SQL with the data bound as a parameter.

- Kind: process
- Verify: check `sql-concatenation`
- Depends: MG-REQ-0176, MG-REQ-0007
- Decision: 0002
