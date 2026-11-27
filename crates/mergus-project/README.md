# mergus-project

Read and validate the project: files, `project.toml`, folder `mergus.toml`.

## What it does

- Walks `models/` through the `FileSystem` port given by the caller and returns one `Model`
  per `.sql` or `.py` file, named after the file stem (MG-REQ-0302).
- Publishes a model at `models/gold/<schema>/<table>` as `gold.<schema>.<table>`; every
  other model is intermediate (MG-REQ-0304).
- Never reads a model file, so no comment, decorator or import has meaning (MG-REQ-0301).
- Rejects duplicate model names with both paths, `MG-0301` (MG-REQ-0303).
- Rejects model, folder, schema, source and seed names that are not lowercase identifiers,
  `MG-0302`, with the suggestion of MG-REQ-0360 (MG-REQ-0305).
- Finds the closest known name for later errors (MG-REQ-0359).
- Reads `project.toml` and each `sources/<source>.toml`, the source named after the file stem
  (MG-REQ-0306), as TOML 1.0 validated by the schemas of `schemas/` of this crate (MG-REQ-0316):
  `MG-0303` to `MG-0305`, with file, line and the closest allowed key.
- Rejects expressions in values, `MG-0306` (MG-REQ-0364), and NaN or infinite floats,
  `MG-0307`; returns each file as JSON and canonical JSON (MG-REQ-0318, MG-REQ-0365).
- Reads every folder `mergus.toml` of `models/` with the folder schema (MG-REQ-0366); a
  section other than `[all]` that names no model of its folder is `MG-0308`, with the closest
  model name (MG-REQ-0331).
- Returns the effective options of each model: its section, `[all]` of its folder, then of
  each parent folder, the convention, the default (MG-REQ-0332); `checks` and `tags` add up
  (MG-REQ-0333); `inherit = false` skips the parents (MG-REQ-0334); every value has its
  origin, file and line, `convention` or `default` (MG-REQ-0335, MG-REQ-0367). A configured
  `publish` overrides the folder rule.
- Gives each model, source, source table and column a stable identifier, a prefix, `_` and 16
  base32 characters (MG-REQ-0368): the one stored for its current name in `ids.toml`
  (MG-REQ-0370), or one derived from its name with SHA-256 (MG-REQ-0369) that no `ids.toml`
  value has. A model moved to another folder keeps it (MG-REQ-0209). An invalid `ids.toml` is
  `MG-0310`, an identifier twice in it `MG-0311`, two objects with one identifier `MG-0312`
  (MG-REQ-0371).
- Returns the resolved references of each `.sql` model, lowercase, a model bare and a raw
  table as `raw.<table>`, sorted (`read_sql_references`, MG-REQ-0373).
- Runs each `macros/*.sql` through the `Database` port in a sandboxed session: a refused
  statement or any object other than a macro is `MG-0604` (MG-REQ-0641); two macros of one name
  or a macro named like a DuckDB built-in, without case, is `MG-0605` (MG-REQ-0642). Gives each
  model the macros it calls, directly or through another macro, as fingerprint inputs
  `macro:<name>` = `sha256:<hex>` (MG-REQ-0644).
- Checks each `seeds/<name>.csv` against its `[seeds.<name>]`: `MG-0606` to `MG-0609`
  (MG-REQ-0645); hashes it for the fingerprints of its readers, and warns with `MG-0610` above
  10,000,000 bytes (MG-REQ-0646).
- Collects every problem, not only the first, each file's problems ordered by line, code and
  key.

## What it does not

- It performs no I/O itself: it calls the port only (MG-REQ-0004).
- It checks no key of `sources/*.toml` yet (MG-REQ-0362.3), and does not know `pii`, `by`,
  `deletes` nor `inputs` yet (MG-REQ-0366).
- It does not analyze SQL or Python, so it does not check that a file produces one table.
- It does not load seeds, nor compute fingerprints: it gives their inputs to the caller.
