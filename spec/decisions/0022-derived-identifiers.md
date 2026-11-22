# 0022 · Identifiers derived from names, renames stored

- Status: accepted
- Date: 2026-11-22
- Answers: phase 0.2 status (#71): MG-REQ-0209 had no storage or format

## Context

MG-REQ-0209 asked for identifiers that survive renames but did not say how they are formed or
where they live. Compiling must stay byte-identical (MG-REQ-0005), so random identifiers
assigned at compile time are excluded, and a model without configuration must not need a
`mergus.toml` section only to hold its identifier.

## Decision

Identifiers are derived from names with SHA-256 (MG-REQ-0369). Model names are unique across
folders, so moves keep them for free. Renames are stored in an optional `ids.toml` at the
project root (MG-REQ-0370), written by `mergus mv` (MG-REQ-0372). Columns cannot be tracked
across a rename without an annotation in code, which MG-REQ-0301 forbids, so a renamed column
is a new column. Runs, publications and diagnostics use `run_id`, `manifest_id` and the error
code.

## Consequences

A rename done outside `mergus mv` (for example `git mv`) gives the model a new identifier
unless the user adds the `ids.toml` entry by hand.
