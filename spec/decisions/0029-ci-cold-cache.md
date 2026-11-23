# 0029 · CI budget on a warm cache, thirty-minute limit

- Status: accepted
- Date: 2026-11-23
- Answers: needs-owner #136 (the owner chose option A)

## Context

Bundled DuckDB (MG-REQ-0193) takes about ten minutes to compile on a cold cache. The jobs'
ten-minute limit cancelled them, and `rust-cache` saves only after success, so the cache could
never warm.

## Decision

MG-REQ-0166 measures the ten-minute budget on a warm cache. The `ci` and `traceability` jobs
get a thirty-minute `timeout-minutes`, so a cold run can finish and save the cache.

## Consequences

An implementation issue labelled `owner-authorized` edits `.github/workflows/ci.yml`. Runs
after a `Cargo.lock` change may be slow once; later runs are back under ten minutes.
