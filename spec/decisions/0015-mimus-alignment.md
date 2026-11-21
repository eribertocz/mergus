# 0015 · Ecosystem alignment: MCP in the repository, actor kind, proposals in the host

- Status: accepted
- Date: 2026-11-21
- Answers: owner decisions relayed by the Mimus specifier

## Context

The owner chose one pattern for the whole ecosystem: each module ships its MCP server inside
its own repository, and every module uses the same actor format. Mimus also asked for a change
proposal contract so that users never see Git.

## Decision

- `mergus-mcp` is a crate of the Mergus workspace, released and tested with the CLI
  (MG-REQ-0290). It runs the `mergus` binary and depends only on `mergus-contracts`.
- `actor` is `{id, kind, on_behalf_of}` with `kind` one of `person`, `agent`, `service`
  (MG-REQ-0264, MG-REQ-1107).
- Change proposals belong to the host: it owns users, approval by a person with permission,
  the Git provider and the merge. Git stays the mechanism of MG-REQ-0191, hidden from the user.
  Mergus gives the host one read-only report per branch, `mergus review <branch> --json`
  (MG-REQ-1317, 0.7).

## Consequences

The host specification needs the proposal contract (propose, show, approve, reject, revert).
