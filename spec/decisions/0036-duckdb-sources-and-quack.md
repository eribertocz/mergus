# 0036 · DuckDB sources through ADBC; Quack later, as a transport

- Status: accepted
- Date: 2026-11-25
- Answers: the owner's decision on DuckDB sources and Quack

## Context

The owner keeps real data in `.duckdb` files, one per year, and asked whether DuckDB, and DuckDB 2.0's
Quack client-server protocol, can be sources and the `server` profile.

## Verified facts

Each fact cites its source. Official documentation first; where the documentation is
silent, the source code that Mergus pins is named as such. A documentation reading session
in the `mergus` environment read the pages, and its quotes are on #172.

From the official documentation:

- DuckDB ADBC (https://duckdb.org/docs/2.0/clients/adbc): the entry point "Must be
  duckdb_adbc_init". The driver option is the "Path to the DuckDB shared library", which
  "contains the core DuckDB engine that the ADBC driver interfaces with". The database
  options are `path` ("If not set, an in-memory database is created") and `uri`. Other
  configuration options, such as `access_mode`, are not documented. The driver "implements
  the full ADBC specification" except partitions and Substrait.
- ADBC versions (https://arrow.apache.org/adbc/main/driver/duckdb.html): "version 1.5.0 and
  later support the ADBC 1.1.0 spec".
- ATTACH (https://duckdb.org/docs/2.0/sql/statements/attach.html): `READ_ONLY` attaches "the
  database in read-only mode".
- Quack (https://duckdb.org/docs/2.0/core_extensions/quack.html, /quack/overview,
  /quack/reference, /quack/security):
  - It is listed under Core Extensions and "is transparently autoinstalled and autoloaded on
    first use".
  - "DuckDB 2.0.0 shipped a polished version of the Quack protocol that is faster and more
    stable."
  - The client uses `ATTACH 'quack:<host>[:port]'`, default port 9494, with a `TOKEN` option
    or a `quack` secret scoped to the server URI.
  - "Non-local URIs default to HTTPS with a self-signed certificate."
  - "Each Quack node exposes a whoami() table macro", whose output carries
    `duckdb_version` and `platform`. "user-supplied keys win on conflict".
- Not stated in the documentation:
  - the minimum DuckDB version for Quack;
  - whether `READ_ONLY` applies to a `quack:` ATTACH;
  - whether a server can expose DuckLake catalogs;
  - opening `quack:` directly as an ADBC or C API path.

From the pinned source code, where the documentation is silent:

- `libduckdb-sys 1.20001.0` bundles DuckDB `v2.0.1` and compiles the ADBC driver
  (`src/common/adbc/adbc.cpp`) into the library that Mergus links statically.
- `adbc_driver_manager 0.25.0` provides `ManagedDriver::load_static(init, version)`.
- `GetInfo` returns `ADBC_INFO_VENDOR_NAME = "duckdb"` and the library version.
- The bundled DuckDB carries `core_functions`, `icu`, `json` and `parquet`. `quack` is
  installed on first use, like `ducklake`.

## Decision

1. DuckDB as a source goes through ADBC, with no exception: `kind = "database"`,
   `engine = "duckdb"`. Its driver is the one compiled into the bundled DuckDB, loaded
   statically, with no download (MG-REQ-0195.4). It is pinned by the `duckdb` crate pin and
   checked through `ADBC_INFO_VENDOR_VERSION`.
2. A `.duckdb` file is declared with `location`, needs no `connection`, and is attached
   with the documented `ATTACH ... (READ_ONLY)` (MG-REQ-0421). Use `mode = "copy"` for raw data, and `mode = "live"` with
   `engine = "duckdb"` for clean tables. One source per file.
3. `engine` now names the ADBC driver of every `database` source: `postgresql` or `duckdb`
   for `copy` (MG-REQ-0409.4).
4. MG-REQ-0410 identifies a local DuckDB by `ADBC_INFO_VENDOR_NAME = "duckdb"`, and a Quack
   server by `whoami()` and its `duckdb_version`. A server can override those keys, so this
   identifies the engine without authenticating it.
5. Quack is a transport of the `duckdb` engine, not a new `kind`, and Mergus is always the
   client. It is a core extension in DuckDB 2.0, which Mergus pins. A Quack source opens an
   in-memory database through the DuckDB ADBC driver and attaches `quack:<host>` with its
   token from the source's secret. That needs `quack` in the extension allowlist of
   MG-REQ-0194. It enters once `READ_ONLY` on a `quack:` ATTACH is documented or proven by
   the conformance tests, because MG-REQ-0429 requires read-only source sessions. Proposed
   phase: 0.7, with environments.
6. The `server` profile uses Quack as its protocol, with Mergus as client only, in the same
   phase as the Quack source. Gold on DuckLake through it is level A, but only once the
   documentation says a Quack server exposes DuckLake catalogs. Gold on a DuckDB file is
   level C+, which stays in phase 1.2.
7. Phase: DuckDB file sources join phase 0.3, right after the ADBC issue (#163). They add no
   download and no new dependency, so their impact is the smallest.

## Consequences

- One issue after #163 implements MG-REQ-0195.4, MG-REQ-0421 and the DuckDB case of
  MG-REQ-0410.
- #152 now also checks MG-REQ-0409.4.
- Quack sources and the `server` profile get their issues in phase 0.7, after the open points above are closed.
