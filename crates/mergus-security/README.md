# mergus-security

Secret resolution, PII handling, hash chains.

## What it does

- Reads a secret reference `secret:<name>`, `<name>` being a valid name (MG-REQ-0305), as a
  `SecretRef`.
- Resolves it by the first place that answers (MG-REQ-1001): the environment variable
  `MERGUS_SECRET_<NAME>`, even when set to an empty value, then the operating system keychain
  under the service `mergus` and the account `<name>`, through the `Environment` and
  `Keychain` ports given by the caller.
- Fails with `SecretError::NotFound` when neither answers. The error names the reference, the
  variable and the keychain entry looked at, and never a value.
- Stores a value for `mergus secret set` with `store`: in the keychain entry of service
  `mergus` and account `<name>`, created or replaced. An empty value fails with
  `StoreError::Empty` (`MG-1001`) and a keychain that cannot be written with
  `StoreError::Unwritable` (`MG-1002`); nothing is stored then, and the error never holds
  the value (MG-REQ-1007). `check` makes the same check without writing, for `--dry-run`. `check` makes the same check without writing, for `--dry-run`.
- Returns every value as a `SecretString` (`secrecy`), which cannot be printed, formatted,
  serialized or logged (MG-REQ-1002).

## What it does not

- It never reads a `.env` file (MG-REQ-1005).
- It has no cloud secret manager; those plugins come after the MVP.
- It does not show connection URLs yet; no connection exists in this phase.
- PII handling and hash chains come with later phases.
