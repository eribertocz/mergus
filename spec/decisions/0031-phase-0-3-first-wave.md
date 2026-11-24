# 0031 · Phase 0.3 first wave: secret names, the keychain port and source keys

- Status: accepted
- Date: 2026-11-24
- Builds on: phase 0.3 issue filing

## Context

Phase 0.3 starts. Its first issues would have to guess observable behavior (MG-REQ-0002):

- MG-REQ-1001 named `MERGUS_SECRET_<NAME>` without saying how `<NAME>` comes from the
  reference, what happens when no link answers, or how the keychain is reached. It also listed
  two links that are not due in phase 0.3.
- `mergus secret set` was mentioned but had no rule.
- No port covered the keychain (MG-REQ-0130).
- The source schema was open (MG-REQ-0362.3), and no rule listed its keys, so the loads and
  source-care rules could not be validated. Files sources had no key saying where the files
  are.

## Decision

- A secret name is a name of MG-REQ-0305. `<NAME>` is that name in ASCII upper case. A set
  variable answers even when it is empty. When no link answers, resolution fails with a typed
  error that names the reference and the places looked at.
- The keychain is reached through a new `Keychain` port, under service `mergus` and account
  `<name>`. Its real implementation uses `keyring` (`architecture/dependencies.md`).
- Cloud secret manager plugins stay after the MVP. Federated identity for object storage
  becomes MG-REQ-1006, in phase 0.9.
- MG-REQ-1007 defines `mergus secret set <name>`: the value comes from standard input, never
  from an argument, so it does not land in shell history.
- MG-REQ-0408 lists the keys of a source file. Files sources get `location` per source, and
  `path` and `format` per table. REST sources stay open until phase 0.8.

## Consequences

The first issues of phase 0.3 (secrets, the adapter contract, source validation) can be
implemented from their context alone. Storage on DuckLake waits for the owner's decision on
how the DuckLake extension is obtained.
