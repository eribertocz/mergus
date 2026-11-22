---
id: project-identifiers
title: Stable identifiers of project objects
crate: mergus-project
phase: "0.2"
status: draft
depends_on: [MG-REQ-0209, MG-REQ-0302, MG-REQ-0303]
---

# Stable identifiers

MG-REQ-0209 asks every named object to carry an identifier that survives renames and moves.
This file defines how the identifiers of project objects are formed and stored. Runs use their
`run_id` (MG-REQ-1101), publications their `manifest_id`, and diagnostics their `MG-####`
code, which `cli/errors.md` never reuses.

Identifiers are derived from names, so a fresh checkout compiles to the same identifiers
without any stored state (MG-REQ-0005). Only renames are stored, in `ids.toml` at the project
root:

```toml
[model]
fact_orders = "mdl_2x7kq4m9a1bc3def"   # renamed from fact_sales

[source]
erp_prod = "src_9hc2v6n0p4qr7stu"      # renamed from erp
```

A model moved to another folder keeps its name (MG-REQ-0303) and so its identifier.

### MG-REQ-0368 · Identifier format

An identifier MUST be a kind prefix, `_`, and 16 characters of lowercase RFC 4648 base32
without padding. The prefixes are `mdl` (model and its table), `src` (source), `stb` (table of
a source) and `col` (column of a model or source table).

- Kind: behavior
- Verify: test `mg_req_0368_identifier_format`

### MG-REQ-0369 · Derived identifiers

When `ids.toml` has no entry for an object, its identifier MUST be the prefix, `_` and the
base32 of the first 10 bytes of the SHA-256 of the UTF-8 text `<prefix>\n<parent>\n<name>\n<n>`.
`<name>` is the model name, source file stem, source table name or column name; `<parent>` is
the identifier of the source (for `stb`) or of the model or source table (for `col`), and is
empty otherwise; `<n>` is the smallest integer from 0 whose result is not a value in
`ids.toml`.

- Kind: behavior
- Verify: test `mg_req_0369_identifier_derived_from_name`

### MG-REQ-0370 · Renames are stored

1. MG-REQ-0370.1: `ids.toml` MAY have the tables `[model]` and `[source]`, each mapping a
   current name to an identifier valid under MG-REQ-0368 with the prefix of its table. Any
   other table, key or value MUST be an error (exit code 1).
2. MG-REQ-0370.2: An object with an entry MUST take the identifier of the entry.
3. MG-REQ-0370.3: The same identifier appearing twice MUST be an error naming both keys.
4. MG-REQ-0370.4: An entry whose name matches no object MUST be kept and its identifier stays
   reserved: no derived identifier may equal it.

- Kind: behavior
- Verify: test `mg_req_0370_renamed_object_keeps_identifier`

### MG-REQ-0371 · Identifiers are never reused

Two objects of the same project MUST NOT share an identifier, and an identifier stored in
`ids.toml` MUST NOT be derived for another object. Two objects resolving to the same
identifier MUST be an error naming both (exit code 1).

- Kind: behavior
- Verify: test `mg_req_0371_identifier_never_reused`

### MG-REQ-0372 · mv records the rename

`mergus mv` MUST write the entry of MG-REQ-0370 under the new name with the identifier the
object had before, and remove the entry of the old name if there was one.

- Kind: behavior
- Verify: test `mg_req_0372_mv_records_rename`
- Phase: 0.8
- Depends: MG-REQ-0354
