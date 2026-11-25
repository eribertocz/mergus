# 0036 · DuckDB sources through ADBC; Quack later, as a transport

- Status: accepted
- Date: 2026-11-25
- Answers: the owner's decision on DuckDB sources and Quack

## Context

The owner keeps real data in `.duckdb` files, one per year, and asked whether DuckDB, and DuckDB 2.0's
Quack client-server protocol, can be sources and the `server` profile.

## Verified facts

These were checked against sources, not assumed. `duckdb.org` and `arrow.apache.org` were
blocked by this session's network, so the facts come from the source code that Mergus
already pins and from the Quack repository on GitHub:

- `libduckdb-sys 1.20001.0` bundles DuckDB `v2.0.1` (its `DUCKDB_VERSION`).
- The bundled source compiles `src/common/adbc/adbc.cpp` into the library. It exports
  `duckdb_adbc_init` and accepts the database options `path` and `uri` (`file:` or
  `duckdb:`). Any other option is passed to DuckDB's configuration, so `access_mode` can be
  set. `GetInfo` returns `ADBC_INFO_VENDOR_NAME = "duckdb"` and
  `ADBC_INFO_VENDOR_VERSION = duckdb_library_version()`.
- `adbc_driver_manager 0.25.0` provides `ManagedDriver::load_static(init, version)`. So the
  driver is used from the linked library, with no file to download. The Rust bindings of
  `libduckdb-sys` do not declare `duckdb_adbc_init`, so `mergus-ports` declares it with FFI
  under MG-REQ-0170.
- The bundled DuckDB carries only `core_functions`, `icu`, `json` and `parquet`.
- Quack is the `quack` extension. Its README says it "is released as a pre-release
  extension and is currently experimental" and that it autoinstalls on first use. The client
  syntax is `ATTACH 'quack:<host>' AS <name>` with a `quack` secret (`TOKEN`, `SCOPE`).
- Not verified: the minimum DuckDB version for Quack, a read-only attach option, opening a
  `quack:` URL directly as an ADBC `path`, and how a client identifies the server.

## Decision

1. DuckDB as a source goes through ADBC, with no exception: `kind = "database"`,
   `engine = "duckdb"`. Its driver is the one compiled into the bundled DuckDB, loaded
   statically, with no download (MG-REQ-0195.4). It is pinned by the `duckdb` crate pin and
   checked through `ADBC_INFO_VENDOR_VERSION`.
2. A `.duckdb` file is declared with `location`, needs no `connection`, and is opened
   read-only (MG-REQ-0421). Use `mode = "copy"` for raw data, and `mode = "live"` with
   `engine = "duckdb"` for clean tables. One source per file.
3. `engine` now names the ADBC driver of every `database` source: `postgresql` or `duckdb`
   for `copy` (MG-REQ-0409.4).
4. MG-REQ-0410 identifies a DuckDB connection by `ADBC_INFO_VENDOR_NAME = "duckdb"`.
5. Quack is a transport of the `duckdb` engine, not a new `kind`. It waits until it leaves
   pre-release and the open points above are verified. Mergus is always the client and
   never starts a server. Quack would need `quack` in the extension allowlist of
   MG-REQ-0194.
6. The `server` profile comes with Quack, after the MVP. Gold on DuckLake through it is
   level A. On a DuckDB file it is level C+, which stays in phase 1.2.
7. Phase: DuckDB file sources join phase 0.3, right after the ADBC issue (#163). They add no
   download and no new dependency, so their impact is the smallest.

## Consequences

- One issue after #163 implements MG-REQ-0195.4, MG-REQ-0421 and the DuckDB case of
  MG-REQ-0410.
- #152 now also checks MG-REQ-0409.4.
- Quack gets its own decision when it is stable.
