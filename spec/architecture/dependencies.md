---
id: architecture-dependencies
title: Dependency policy and approved libraries
crate: none
phase: "0.1"
status: draft
depends_on: [architecture-boundaries]
---

# Dependencies

| Purpose | Library | Purpose | Library |
|---|---|---|---|
| Data | arrow-rs, duckdb-rs, adbc | Configuration | toml |
| Polars | polars (pinned per release) | Contract validation | jsonschema |
| Python bridge | PyO3, maturin | Contract types | typify |
| Python environments | uv | Hashes | sha2 + own RFC 8785 |
| CLI | clap, clap_complete | Errors in terminal | annotate-snippets |
| Progress | indicatif | Logs and telemetry | tracing, opentelemetry |
| HTTP server (server mode) | axum | TLS | rustls |
| Secrets in memory | secrecy | OS keychain | keyring (watched) |
| Tests | insta, proptest (watched), nextest | SQL tests | sqllogictest-rs |
| Embedded UI assets | rust-embed (watched) | Permissions (server mode) | Cedar |

Rejected: miette (low maintenance), any YAML crate (configuration is TOML), utoipa (contract
first instead), third-party RFC 8785 crates (own certified implementation).

### MG-REQ-0185 · Dependency criteria

A new dependency MUST be backed by a foundation, a company or a team with several active
maintainers, and have a release in the last six months. Small critical pieces (such as RFC
8785) MUST be written in-house and certified against their reference vectors.

- Kind: process
- Verify: review `dependency-criteria`

### MG-REQ-0186 · Behind an own interface

Every external library used by more than one crate MUST be wrapped behind an interface owned
by Mergus, so it can be replaced without touching callers.

- Kind: process
- Verify: review `dependency-wrapped`

### MG-REQ-0187 · Watched dependencies reviewed quarterly

Dependencies marked "watched" MUST be reviewed every quarter, with a replacement plan
recorded in a decision record when their health degrades.

- Kind: process
- Verify: review `watched-dependencies`

### MG-REQ-0188 · Contract-first HTTP API

The server-mode HTTP API MUST be defined in OpenAPI and JSON Schema first; Rust and
TypeScript types are generated from it.

- Kind: process
- Verify: review `api-contract-first`
- Phase: 1.1
