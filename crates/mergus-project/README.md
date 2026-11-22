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
- Collects every problem, not only the first.

## What it does not

- It performs no I/O itself: it calls the port only (MG-REQ-0004).
- It does not read TOML files yet: `project.toml`, `mergus.toml` and `sources/*.toml`, and
  the `publish` option, come with later issues.
- It does not analyze SQL or Python, so it does not check that a file produces one table.
