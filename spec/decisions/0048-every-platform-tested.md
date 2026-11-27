# 0048 · Every shipped platform is tested, and outputs match across platforms

- Status: accepted
- Date: 2026-11-27
- Answers: the owner's request for an ideal, truly cross-platform Mergus

## Context

MG-REQ-1601 ships five binaries: Linux x86_64 and arm64 (musl), macOS arm64 and x86_64, and
Windows x64. CI tests only three systems, one architecture each, in debug builds. Two shipped
platforms, Linux arm64 and macOS x86_64, are never tested. No test checks that the same input
gives the same bytes on every platform, although Mergus promises reproducible manifests and
pin recipes to its consumers (Accipiter).

## Decision

- **Pull requests (MG-REQ-1606.1):** each one keeps the three-system matrix, so the cost per
  pull request does not grow.
- **Nightlies and releases (MG-REQ-1606.2):** they add Linux arm64 and macOS x86_64, plus a
  smoke test of each release binary itself, the static musl build included.
- **Failing platform (MG-REQ-1606.3):** a release with a failing platform publishes nowhere.
- **Determinism (MG-REQ-0149):** the determinism test runs on every platform and compares the
  outputs with those of Linux x86_64, byte for byte.

## Consequences

- A platform-specific bug cannot reach a release unseen.
- The new check `platform-matrix` comes with phase 0.9 (distribution). The cross-platform part
  of MG-REQ-0149 comes with its phase, 0.5.
