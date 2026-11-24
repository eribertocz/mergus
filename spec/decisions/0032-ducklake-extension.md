# 0032 · DuckLake through a Database port, extension pinned and cached

- Status: accepted
- Date: 2026-11-24
- Answers: needs-owner #147 (the owner chose option A)

## Context

Storage in phase 0.3 is DuckLake, a DuckDB extension that the bundled DuckDB lacks. DuckDB
was used only by the `SqlParser` port, but the DuckLake adapter and the DuckDB engine of
phase 0.4 also need it. MG-REQ-0186 requires an external library used by several crates to be
wrapped behind a Mergus interface.

## Decision

- The pinned `duckdb` crate moves to a release whose DuckDB supports DuckLake.
- A new `Database` port owns every DuckDB connection. No crate other than `mergus-ports`
  depends on `duckdb` (MG-REQ-0194).
- Option A of #147. On first use, the real implementation installs only `ducklake`, at the
  version that matches its DuckDB, from `extensions.duckdb.org` with signature checks. It
  caches the extension in the user cache directory and uses it offline after that. Autoload
  and unsigned extensions stay off.
- The simulated `Database` is an in-memory DuckDB without extensions. Simulator tests of the
  adapter run on DuckDB tables; the DuckLake behavior is tested on the real port.

## Consequences

CI needs `extensions.duckdb.org`, which GitHub runners reach. Upgrading `duckdb` may change
the cargo-deny exceptions of decision 0028; that change is an owner file and needs the label
`owner-authorized`. `mergus doctor` reporting the extension comes with the doctor issue.
