# 0044 · Shape of the raw tables

- Status: accepted
- Date: 2026-11-26
- Answers: spec-defect #217 (issue #194)

## Context

Issue #194 implements MG-REQ-0511, 0512, 0515 and 0518. The specification did not give:

- the types of the metadata columns;
- the values of a delete row;
- what a load with a repeated key does;
- whether the current table is a view or a table;
- the error codes of MG-REQ-0515.

It also did not say what happens when a `local` or `shared` raw catalog falls inside the
project.

Facts checked by the implementer on DuckDB 2.0.1 and DuckLake:

- `ENCRYPTED` writes need `httpfs` (MG-REQ-0194 allows it for this).
- Attaching an unencrypted catalog with `ENCRYPTED` fails.

## Decision

- **Metadata types:** `_load_id VARCHAR`; `_extracted_at` and `_deleted_at` `TIMESTAMPTZ` in
  UTC; `_deleted BOOLEAN NOT NULL`.
- **Delete rows:** a detected delete carries the last known values. Its `_load_id` and
  `_extracted_at` are those of the detecting load, and `_deleted_at` is that load's
  `_extracted_at`. History models (SCD2) need the last values; NULL columns would lose them.
- **Repeated key:** a load that returns a key twice fails with a code and writes nothing. A
  "last row wins" rule would depend on read order and break determinism (MG-REQ-0149).
- **Current table:** current and changes are both tables written in the same adapter commit,
  so they are atomic. The current table is rewritten with `replace`. The adapter contract keeps
  its six operations (MG-REQ-0285); a view operation is not added.
- **Catalog placement:** MG-REQ-0515.2 (decision 0041) is applied as written. A `local` or
  `shared` raw catalog inside the project root is refused with a code before anything is
  attached.
- **Error codes:** the new failures take the next free `MG-05xx` codes, with catalog files.

## Consequences

- Each load rewrites the current rows of a table. A cheaper incremental form, such as a view
  operation or a merge that can delete, is a later change to the adapter contract.
- `[raw]` of a `local` or `shared` project needs a `location` outside the project. The error's
  fix says so.
