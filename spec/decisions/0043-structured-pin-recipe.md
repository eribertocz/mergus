# 0043 · The pin recipe is a structured object

- Status: accepted
- Date: 2026-11-26
- Answers: the owner's request to adapt Mergus to Accipiter, a consumer that reads gold

## Context

MG-REQ-0249 put "the pin recipe returned by its adapter" in each manifest entry but did not
say what it holds. The DuckLake adapter in review (#196) returns free text
(`ducklake snapshot <id>`), which a consumer such as Accipiter would have to parse.

## Decision

- `pin_recipe` returns the canonical JSON of `{format, table, version}` (MG-REQ-0298), carried
  in the existing `PinRecipe` text type, so the adapter trait does not change.
- The recipe never holds where the store is or any credential. Where a table's store is and
  which engine reads it is given by the manifest entry, specified with the manifest in phase
  0.5, together with the manifest JSON Schema.

## Consequences

- #196 returns the structured recipe now.
- The manifest schema of phase 0.5 embeds the recipe object as `$defs.pin_recipe`.
