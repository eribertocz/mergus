# 0038 · The PostgreSQL driver comes from the Apache ADBC release on GitHub

- Status: accepted
- Date: 2026-11-25
- Answers: #163; the owner's choice of the official Apache release

## Context

MG-REQ-0195.1 pins each fetched ADBC driver by URL and SHA-256, but did not say which
artifact holds the library. Apache publishes no bare shared library for the PostgreSQL driver:
each operating system and architecture gets a Python wheel. The other channels considered,
conda-forge (a community repackaging) and `dbc` (Columnar, with no published sums or
pinned versions), add a third party between Apache and Mergus.

## Decision

The driver is the wheel of the Apache Arrow ADBC release on GitHub, pinned by its SHA-256.
Mergus verifies the wheel, then extracts the one member named `libadbc_driver_postgresql`
with the platform's library extension. PyPI's description of the package names the library
`libadbc_driver_postgresql.{dll,dylib,so}`; the path inside the wheel is not documented,
so Mergus matches the file name, not a path.

Pins for ADBC 25 (package version 1.13.0, tag `apache-arrow-adbc-25`), with URLs under
`https://github.com/apache/arrow-adbc/releases/download/apache-arrow-adbc-25/`. Each SHA-256 was read on the GitHub release page and on the PyPI JSON API
(https://pypi.org/pypi/adbc-driver-postgresql/1.13.0/json), and the two agree:

| Target | Wheel | Size (bytes) | SHA-256 |
|---|---|---|---|
| Linux x86_64 | `adbc_driver_postgresql-1.13.0-py3-none-manylinux_2_26_x86_64.manylinux_2_28_x86_64.whl` | 3560463 | `a86819ba7c79b1d067c4a5344358b4d346bab58e888d7879ed2b25d97cc59ba1` |
| Linux aarch64 | `adbc_driver_postgresql-1.13.0-py3-none-manylinux_2_26_aarch64.manylinux_2_28_aarch64.whl` | 3871372 | `f2f3c51076fc27ba768bad2dc4cc3888655ac719473aa7d933b2813860491014` |
| macOS x86_64 | `adbc_driver_postgresql-1.13.0-py3-none-macosx_10_15_x86_64.whl` | 3076900 | `12fd64bf6ec16705d30d26c38ac2cc47fe8c7006ac2902c43234d61cb6ae8a43` |
| macOS arm64 | `adbc_driver_postgresql-1.13.0-py3-none-macosx_11_0_arm64.whl` | 3348673 | `7981ae7c5ecd7ba4c649cd00a96e81cf0e39475d3fa472901c5fd19b4e9de4a8` |
| Windows x86_64 | `adbc_driver_postgresql-1.13.0-py3-none-win_amd64.whl` | 3111598 | `c97a0b71e7205b7aac7f8bcd85a41280ef56e2d788110d2572c52ac3d447e25d` |
| Windows arm64 | `adbc_driver_postgresql-1.13.0-py3-none-win_arm64.whl` | 3414650 | `ed770fddf8b5fedf98e36b728224f1a5df3c0e1daaea1abc6feb41a765c7aa79` |

A checksum mismatch fails with its own typed error, separate from the missing driver of
MG-REQ-0195.3, naming the driver, the URL, the expected and the actual sum. Both take codes
of the extract area (`MG-04xx`, next free numbers).

## Consequences

- No third party sits between Apache and Mergus; a new ADBC release means new pins in a
  Mergus release, verified the same way.
- The Linux wheels need glibc 2.28 or later, as their tags say.
- Reading a zip archive needs a zip crate in `mergus-ports`, recorded in its pull request.
