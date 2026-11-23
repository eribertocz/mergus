# 0028 · Scoped cargo-deny exceptions for the duckdb crate

- Status: accepted
- Date: 2026-11-23
- Answers: needs-owner #123 (the owner chose option A), spec-defect #122

## Context

The `duckdb` crate, needed by MG-REQ-0193 and by the DuckDB engine of phase 0.4, always pulls
`arrow`. Through it, `tiny-keccak` is licensed `CC0-1.0` and `chrono` 0.4.45 has the
unmaintained notice RUSTSEC-2026-0543 with no safe upgrade, so `cargo deny` fails.

## Decision

Two exceptions, each scoped and written with its reason in `deny.toml` (MG-REQ-1746.5), as
Menura does for the same crate: `CC0-1.0` for `tiny-keccak` only, and an ignore of
RUSTSEC-2026-0543. The license list of MG-REQ-1746.1 does not change. Option B (`libduckdb-sys`
with in-house FFI) was rejected: it needs `unsafe` (MG-REQ-0170) and more code to maintain.

## Consequences

An implementation issue labelled `owner-authorized` edits `deny.toml`; the real `SqlParser`
follows. Each exception is removed when its "remove when" condition holds.
