---
id: roadmap
title: Phases to the MVP and after, with their gates
crate: none
phase: "0.1"
status: draft
depends_on: [governance-workflow]
---

# Roadmap

For the specifier, the release preparer and the owner. Implementers never read it.

Current phase: 0.2

Every rule belongs to a phase: the `phase` of its file, or its own `Phase`. `just spec-status`
prints every rule with its phase and state. The specifier files the issues of a phase with the
`create-issues` skill; a phase is released when all its gates pass.

## 1. Phases to the MVP

| Phase | Goal | Areas (files whose rules are due) | Usable result | Menura needed |
|---|---|---|---|---|
| 0.1 | Foundations | kernel, architecture, contracts/overview, governance | Repository, CI guards, spec tool, ports, simulator skeleton, contract schemas | — |
| 0.2 | Read and plan | project (layout, TOML), run/plan, cli (commands, output, errors), contracts/events | `mergus validate`, `plan`, `ls`, `--json` | — |
| 0.3 | Extract | extract (sources, loads, source care), storage, contracts/adapter, security/secrets | Sources land in encrypted raw on DuckLake; resume works | — |
| 0.4 | Transform | model, run (stages, gates) | SQL and Python models run end to end into work | — |
| 0.5 | Test and publish | quality, publish, contracts (manifest, Menura catalog and inputs) | First verified publication with catalog validated by Menura; `init --demo` | 0.2.0 (schema); 0.6.0 for state and summaries |
| 0.6 | Protect | security (identities, PII, protection) | PII handled at the door, gold gate, audit chain | — |
| 0.7 | Operate | observability, environments, run/scheduling, contracts/ops-tables | `status`, `lineage`, environments, `promote`, `rollback`, `schedule` | 0.7.0 for `describe`/`explain` links |
| 0.8 | Reach | extract/rest, project/tools, governance/docs, MCP adapter (MG-REQ-0290) | REST APIs, project tools, documentation site, agent guide, MCP adapter | — |
| 0.9 | Harden and ship | ops, distribution, cli/performance, governance/privacy | Recovery, upgrades, signed channels, performance targets met | — |
| **1.0** | **MVP** | No new rules | Release candidate becomes 1.0.0 | 1.0.0 (`menura.*` names) |

## 2. Gates of every phase

A phase is released only when every gate passes in CI on `main`.

| Gate | Criterion |
|---|---|
| Traceability | Every `behavior` rule of the phase and earlier phases has a passing test (`just spec-status` shows `tested`) |
| Process | Every `process` rule of the phase has its check running or its review item in `review-pr` |
| Invariants | Every invariant due by this phase holds over 1,000 simulator seeds per pull request and 24 hours of continuous simulation |
| Determinism | Plans and manifests byte-identical across two runs with shuffled input order (MG-REQ-0005) |
| Compatibility | Every contract example validates; schema changes bump versions (MG-REQ-0205) |
| Speed | Pull request CI under ten minutes (MG-REQ-0166) |

## 3. Additional gates of 1.0

| Gate | Criterion |
|---|---|
| Completeness | Every rule with phase ≤ 0.9 is `tested`, `by check` or `by review`, none `pending` |
| End to end | Nightly end-to-end suite green for seven consecutive days |
| Performance | Every target of `cli/performance.md` met on the reference machines |
| Security | Threat model reviewed; `doctor --security` clean on the reference deployment |
| Menura | CI matrix green against Menura 1.0.0 with `menura.*` contract names |
| Distribution | Signed release published on every MVP channel; `uvx mergus init --demo` works on a clean machine |
| Documentation | Tutorials, guides, reference and explanations complete; examples run |

## 4. After the MVP, by priority

| Phase | Content |
|---|---|
| 1.1 | UI (observe, then operate), server-mode sandbox (MG-REQ-1051), `forget` (MG-REQ-1056) |
| 1.2 | Iceberg and Delta adapters; levels B, C, C+; quarantine action; quality score |
| 1.3 | CDC sources; cloud secret managers; Airbyte declarative manifests; Python escape for REST |
| 1.4 | Editor extension; HTTP API for server mode |
| 1.5 | ClickHouse adapter; triggers on other projects' publications; shared packages; translations |
| later | LTS channel; WinGet, Scoop, apt/rpm, conda-forge, Nix, Helm; browser playground |

Post-MVP items become rules with `Phase: 1.x` when the specifier details them.

## 5. External dependencies

| Need | From | Version | Affects |
|---|---|---|---|
| `validation_report` schema and CLI output | Menura | 0.2.0 | Schema gate (MG-REQ-0232) |
| Official type mapping JSON, invalid examples, producer suite | Menura | 0.2.0 | MG-REQ-0215, CI |
| `producer_requirements` and its command, summary build SQL, time zone | Menura | 0.6.0 | MG-REQ-0237 |
| Summary without catalog `source` does not reject the model; expression coverage; several grains | Menura | 0.6.0 | Summaries |
| Checks that fail to run are omitted | Menura | 0.7.0 | MG-REQ-0235 |
| `describe`, `explain`, column lineage | Menura | 0.7.0 | UI cross-navigation |
| Contract-name aliases, then `menura.*` only | Menura | alias after 0.2.0, removed in 1.0.0 | MG-REQ-0217 |
| Model-dependencies contract (metric → fields → tables, per user) | Menura | to be specified | MG-REQ-1423 |
| Validity columns of history tables in the catalog | Menura | to be specified | MG-REQ-0663 |
| ClickHouse dialect and pinned sessions | Menura | 1.1.0 | Level C |
| UI manifest, message protocol, W3C tokens, search index, stable references | Host / distribution | not scheduled | `ui/integration.md` |
| Host and executor specifications | Ecosystem | not scheduled | `contracts/consumers.md` |
