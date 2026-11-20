---
id: MAP
title: Where each topic lives
crate: none
phase: "0.1"
status: draft
depends_on: []
---

# Map

For the specifier. Find the area of your change, then open only its files.
Implementers never use this map: they use `just spec-context`.

| If the work is about | Area | Files |
|---|---|---|
| Rules every session follows | — | [kernel.md](kernel.md) |
| How the specification is written | — | [README.md](README.md) |
| Roles, issues, pull requests, reviews, releases | governance | [workflow](governance/workflow.md), [enforcement](governance/enforcement.md) |
| Vocabulary | — | [glossary.md](glossary.md) |
| What Mergus does, data flow, neighbouring modules | architecture | [overview](architecture/overview.md) |
| Crates and repository layout | architecture | [crates](architecture/crates.md) |
| Dependency direction, single writers | architecture | [boundaries](architecture/boundaries.md) |
| I/O isolation | architecture | [ports](architecture/ports.md) |
| Invariants INV-01 to INV-10 | architecture | [invariants](architecture/invariants.md) |
| Test levels, simulator, traceability | architecture | [testing](architecture/testing.md) |
| Code style rules | architecture | [code](architecture/code.md) |
| Product guarantees (correctness, measured efficiency, optional AI, owners) | architecture | [principles](architecture/principles.md) |
| Which contracts exist, versioning, compatibility | contracts | [overview](contracts/overview.md) |
| catalog_schema / catalog_state for Menura, tokens, type mapping | contracts | [menura-catalog](contracts/menura-catalog.md) |
| validate, producer_requirements, integrity checks, schema gate | contracts | [menura-inputs](contracts/menura-inputs.md) |
| Manifest, pointer, catalog layout | contracts | [manifest](contracts/manifest.md) |
| `--json` events, exit codes | contracts | [events](contracts/events.md) |
| Ops tables | contracts | [ops-tables](contracts/ops-tables.md) |
| Storage adapter trait, conformance kit | contracts | [adapter](contracts/adapter.md) |
| Project folders, model files, naming, publication by folder | project | [layout](project/layout.md) |
| `project.toml`, config validation, version pin, literal secrets | project | [project-toml](project/project-toml.md) |
| Folder `mergus.toml`, model options, precedence | project | [folder-config](project/folder-config.md) |
| explain, config, suggest, mv, fmt, contract lock | project | [tools](project/tools.md) |
| `sources/*.toml`, modes, raw names, source schema changes | extract | [sources](extract/sources.md) |
| Full and incremental loads, overlap, deletes, resume, snapshots | extract | [loads](extract/loads.md) |
| Replica, load budget, brake, window, index check | extract | [source-care](extract/source-care.md) |
| REST APIs | extract | [rest](extract/rest.md) |
| Zones raw / work / ops / gold | storage | [zones](storage/zones.md) |
| Raw layer | storage | [raw](storage/raw.md) |
| Storage profiles and guarantee levels | storage | [profiles](storage/profiles.md) |
| SQL models | model | [sql](model/sql.md) |
| Python models, steps, engines by return | model | [python](model/python.md) |
| Segments, boundaries, Polars levels, non-determinism | model | [engines](model/engines.md) |
| Macros and seeds | model | [reuse](model/reuse.md) |
| full / incremental refresh, lookback, materialize, backfill | model | [refresh](model/refresh.md) |
| History tables (SCD2) | model | [history](model/history.md) |
| Six stages, failures, resume, cancel, finalizer | run | [stages](run/stages.md) |
| Fingerprints, plan artifact | run | [plan](run/plan.md) |
| Gates before execution | run | [gates](run/gates.md) |
| Freshness, triggers, lock, retries, why | run | [scheduling](run/scheduling.md) |
| Tests, actions, locked output | quality | [tests](quality/tests.md) |
| How tests run, monitors, profiles | quality | [evaluation](quality/evaluation.md) |
| Publication order, all-or-nothing | publish | [sequence](publish/sequence.md) |
| Publication levels, safe degradation | publish | [levels](publish/levels.md) |
| Secret references and resolution | security | [secrets](security/secrets.md) |
| Four identities, least privilege | security | [identities](security/identities.md) |
| PII decisions, detector, labels, gold gate | security | [pii](security/pii.md) |
| Sandbox, TLS, encryption, audit chain, forget, supply chain | security | [protection](security/protection.md) |
| run_id, journal, retention, run report | observability | [journal](observability/journal.md) |
| Status, lineage, audit at time, alerts, OTel, OpenLineage | observability | [insight](observability/insight.md) |
| Environments, physical tables, dev sample | environments | [environments](environments/environments.md) |
| diff, promote, rollback | environments | [promotion](environments/promotion.md) |
| Command map, init, selection, confirmations, precedence | cli | [commands](cli/commands.md) |
| Output modes and error layout | cli | [output](cli/output.md) |
| Error code catalog | cli | [errors](cli/errors.md) |
| Performance targets | cli | [performance](cli/performance.md) |
| UI architecture and ecosystem rule (post-MVP) | ui | [integration](ui/integration.md) |
| UI sections, capabilities, identity (post-MVP) | ui | [experience](ui/experience.md) |
| Resource governor | ops | [resources](ops/resources.md) |
| Backups and recovery | ops | [recovery](ops/recovery.md) |
| Migrations and upgrades | ops | [upgrades](ops/upgrades.md) |
| Platforms and channels | distribution | [channels](distribution/channels.md) |
| Installer, version manager, adapters, offline | distribution | [install](distribution/install.md) |
| License, DCO, trademark | governance | [license](governance/license.md) |
| Zero telemetry | governance | [privacy](governance/privacy.md) |
| User documentation | governance | [docs](governance/docs.md) |
| Roadmap: phases, gates, post-MVP | — | [roadmap.md](roadmap.md) |
| Dependency policy and approved libraries | architecture | [dependencies](architecture/dependencies.md) |
| What host and executor rely on (informative) | contracts | [consumers](contracts/consumers.md) |
