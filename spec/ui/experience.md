---
id: ui-experience
title: Sections, capabilities, identity and quality of the UI
crate: none
phase: "1.1"
status: draft
depends_on: [ui-integration]
---

# UI experience

| Section | Answers |
|---|---|
| Home | Is everything OK? Only what has a problem, with cause and action |
| Map | Where? Graph sources → raw → models → gold → Menura with lenses (health, freshness, duration, cost, PII) |
| Runs | What is happening now? Live timeline of stages and segments |
| Environments | What differs? Model, schema and value-level data diffs; promote and roll back |
| Incidents | What do I do? One page per problem, closing itself when solved |
| Settings | Is it safe? Permissions review, secret origins, PII queue |

### MG-REQ-1421 · Six sections, no submenus

The navigation MUST have exactly the six sections of this table, without submenus.

- Kind: behavior
- Verify: review `ui-six-sections`

### MG-REQ-1422 · Time machine

A time control MUST render every view as it was at a chosen time, from pinned manifests.

- Kind: behavior
- Verify: test `mg_req_1422_time_machine_uses_manifests`

### MG-REQ-1423 · Impact radius before acting

Before promote, rollback or backfill, the UI MUST show which metrics and dashboards change
and by how much, filtered by user. Requires Menura's model-dependencies contract.

- Kind: behavior
- Verify: test `mg_req_1423_impact_radius_filtered`

### MG-REQ-1424 · Trust page and badges

The UI MUST offer a business-facing trust page per area and SVG badges
(`/badge/<stable-ref>.svg`) that respect permissions.

- Kind: behavior
- Verify: test `mg_req_1424_trust_page_and_badges`

### MG-REQ-1425 · Plain explanations without AI

Plain-language explanations MUST come from deterministic templates, never from AI or
external services.

- Kind: behavior
- Verify: review `ui-template-explanations`

### MG-REQ-1426 · Responsiveness and accessibility

Interactions MUST stay under 200 ms INP (internal target 100 ms), first load under 1 s, fluid
with 10,000 models; the UI MUST meet WCAG 2.2 AA, show states with color, icon and text,
offer a table alternative to the graph, and be fully usable by keyboard.

- Kind: behavior
- Verify: test `mg_req_1426_ui_performance_and_accessibility`

### MG-REQ-1427 · Visual identity

Standalone mode MUST use the Mergus identity (logo with the merging crest, palette Merganser
Teal, Serrator Red, Rust, Bone, Ink; Space Grotesk, Inter, JetBrains Mono; dark, light, high
contrast and system themes) expressed only as design tokens. The final logo and mockups are
designed during implementation.

- Kind: behavior
- Verify: review `ui-identity-tokens`
