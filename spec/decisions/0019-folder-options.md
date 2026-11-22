# 0019 · Folder options of phase 0.2 and their origins

- Status: accepted
- Date: 2026-11-22
- Answers: spec-defect #92 (blocked issue #78); open questions of #79

## Context

MG-REQ-0362.2 pointed at the option table of `project/folder-config.md`, outside the context
of #78, and the allowed keywords could not express one table per model. The table gave no
format for durations, grains or published names, and MG-REQ-0335 no shape for an origin.

## Decision

- Configuration schemas may use `patternProperties` for tables keyed by a name (MG-REQ-0361).
- MG-REQ-0366 lists the folder options and values of phase 0.2: durations `<n>m|h|d`, grains
  `day` to `year`, `publish` as `false` or `gold.<schema>.<table>`, `inherit` in model tables
  only. `pii`, `by`, `deletes` and `inputs` come with their phases.
- MG-REQ-0367: an origin is a file and line, `convention` or `default`.
- The folder schema moves from #78 to #79, which has the folder rules in its context.

## Consequences

#78 covers `project.schema.json` and `source.schema.json`; #79 adds `folder.schema.json`.
