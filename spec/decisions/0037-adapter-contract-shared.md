# 0037 · The storage adapter contract is a shared layer-3 crate

- Status: accepted
- Date: 2026-11-25
- Answers: #177

## Context

`mergus-adapter` holds the `StorageAdapter` contract. `mergus-adapter-ducklake` implements
it, `mergus-certify` tests adapters against it, and other layer-3 crates may call an
adapter. All of them are layer 3, and MG-REQ-0120 let a layer-3 crate depend only on the
shared crates `mergus-security` and `mergus-ops`, so the boundary check refused every one
of those dependencies.

## Decision

`mergus-adapter` becomes a shared layer-3 crate, next to `mergus-security` and
`mergus-ops` (MG-REQ-0120). Its layer stays 3, so MG-REQ-0121 and MG-REQ-0126.6 still keep
it out of `mergus-plan`'s reach.

Rejected: moving the contract to layer 2 would put it beside `mergus-plan` and blur what
the plan may reach. A special rule for `mergus-adapter-*` alone would leave `mergus-certify`
and other callers blocked.

## Consequences

- Any layer-3 crate may depend on the adapter contract. The cycle check still applies, so
  `mergus-adapter` itself cannot depend back on a crate that uses it.
- The boundary check's list of shared layer-3 crates gains `mergus-adapter`, through an
  implementation issue that #158 waits for.
