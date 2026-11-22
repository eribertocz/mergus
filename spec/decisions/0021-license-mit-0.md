# 0021 · MIT-0 is an allowed dependency license

- Status: accepted
- Date: 2026-11-22
- Answers: needs-owner #97 (the owner approved option 1)

## Context

`jsonschema` 0.26 and later depend on `borrow-or-share`, licensed MIT-0, which MG-REQ-1746.1
did not list, so #78 had to pin an old `jsonschema`.

## Decision

`MIT-0` joins the list of MG-REQ-1746.1. It is OSI-approved and more permissive than MIT.

## Consequences

An implementation issue with the label `owner-authorized` adds it to `deny.toml`; the pin of
`jsonschema` can then be lifted.
