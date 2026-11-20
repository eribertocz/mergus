---
id: architecture-overview
title: What Mergus does and how data flows through it
crate: none
phase: "0.1"
status: draft
depends_on: [kernel]
---

# Overview

## 1. Purpose

Mergus moves data from operational sources to analytical tables that people and the
semantic compiler (Menura) can trust. The user writes SQL (DuckDB dialect), Python (Polars)
and TOML; never Rust.

## 2. Flow

```
sources ──extract──▶ raw ──models──▶ intermediate ──models──▶ gold ──publish──▶ catalog
 (DB, files, APIs)   (encrypted,      (any table not          (models/gold/...)   + manifest
                      append-only)     under models/gold)                          + pointer
                                     ▲ quality gates run before anything is published ▲
```

| Stage | Input | Output | Area |
|---|---|---|---|
| Extract | Source declarations, secrets by reference | `raw.<source>__<table>` and `raw.<source>__<table>__changes` | `extract/` |
| Model | `.sql`, `.py`, folder `mergus.toml` | Intermediate and gold tables | `model/` |
| Run | Project, selection | Plan, then execution in DuckDB and Polars | `run/` |
| Quality | Tests and contracts | Gate decision: publish or stop | `quality/` |
| Publish | Gold tables that passed the gates | Manifest, pointer move, catalog for Menura | `publish/` |

## 3. The modules around Mergus

| Module | Relation | Through |
|---|---|---|
| Menura (semantic compiler) | Consumes Mergus's catalog; tells Mergus what it requires | `contracts/` documents only |
| Host (application shell) | Embeds Mergus's UI pieces; reads manifests and ops tables | UI manifest, contracts |
| Executor | Runs Mergus on a schedule or on demand | CLI, exit codes, `--json` events |
| BI tools | Read gold tables and ops tables | SQL over the published storage |

### MG-REQ-0101 · Users never write Rust

Every user-facing extension point MUST be expressible in SQL (DuckDB dialect), Python
(Polars) or TOML. No feature MAY require the user to write, compile or load Rust code,
except authors of third-party adapters (`contracts/adapter.md`).

- Kind: behavior
- Verify: review `no-rust-for-users`

### MG-REQ-0102 · Nothing half-published is visible

A reader MUST see either the complete previous publication or the complete new one, never a
mixture. Publication becomes visible only by moving the environment pointer (INV-01).

- Kind: behavior
- Verify: test `mg_req_0102_reader_never_sees_partial_publication`
- Phase: 0.5
- Depends: MG-REQ-0140

### MG-REQ-0103 · Gold is the only published layer

Only tables under `models/gold/<schema>/<table>` MUST be published, as
`gold.<schema>.<table>`. Raw and intermediate tables MUST NOT appear in the catalog given to
Menura.

- Kind: behavior
- Verify: test `mg_req_0103_only_gold_in_catalog`
- Phase: 0.5

### MG-REQ-0104 · One binary

Mergus MUST ship as one executable `mergus`. Python is required only when the project
contains `.py` models; adapters are fetched on demand (`distribution/`).

- Kind: behavior
- Verify: check `release-artifacts`
- Phase: 0.9
