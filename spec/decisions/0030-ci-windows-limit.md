# 0030 · Sixty-minute cold-cache limit on Windows

- Status: accepted
- Date: 2026-11-23
- Answers: needs-owner #140 (the owner chose option A)

## Context

With the thirty minutes of decision 0029, Linux and macOS compiled bundled DuckDB and saved
their cache, but MSVC on Windows did not finish, so its cache could never warm.

## Decision

The `ci` job on `windows-latest` gets a sixty-minute `timeout-minutes`; the other systems keep
thirty. Skipping the parser tests on Windows was rejected (MG-REQ-0013, MG-REQ-0189).

## Consequences

An implementation issue labelled `owner-authorized` edits `.github/workflows/ci.yml`.
