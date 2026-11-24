# 0032 · DuckLake through a Database port, extension pinned and cached

- Status: accepted
- Date: 2026-11-24
- Answers: needs-owner #147 (the owner chose option A), spec-defects #154 and #155

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
- The simulated `Database` answers from recorded results, like the simulated `SqlParser`,
  so `mergus-sim` needs no `duckdb`. The DuckLake behavior is tested on the real port, which
  tests enable as a dev-dependency.

- The real `Keychain` lives in `mergus-ports` under `real`, like every real port
  (MG-REQ-0133), and its simulated one in `mergus-sim` (MG-REQ-0131). MG-REQ-0126.7 now lets
  only `mergus-ports` depend on `keyring` and `duckdb`. `mergus-security` still owns resolution
  (MG-REQ-0123) and the `secrecy` types (#154).
- MG-REQ-0376 depends on MG-REQ-1001, so its context holds the grammar of a secret
  reference (#155).

## Consequences

CI needs `extensions.duckdb.org`, which GitHub runners reach. Upgrading `duckdb` may change
the cargo-deny exceptions of decision 0028; that change is an owner file and needs the label
`owner-authorized`. `mergus doctor` reporting the extension comes with the doctor issue.
