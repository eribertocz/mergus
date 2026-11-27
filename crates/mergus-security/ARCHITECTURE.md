# mergus-security architecture

## Main types

| Type | Role |
|---|---|
| `SecretRef` | A reference `secret:<name>`; gives the name and the variable `MERGUS_SECRET_<NAME>` |
| `resolve` | The resolution chain of MG-REQ-1001: variable, then keychain |
| `store` | Writes a value into the keychain entry of a reference (MG-REQ-1007) |
| `origin` | The same chain as `resolve`; returns the link that answered, never the value (MG-REQ-1004) |
| `SecretOrigin` | `Environment` (the variable) or `Keychain` (the account): shown as `doctor` prints it |
| `check` | The checks of `store`, without writing: for `secret set --dry-run` |
| `StoreError` | `Empty` (`MG-1001`) or `Unwritable` (`MG-1002`): the reference, never a value |
| `KEYCHAIN_SERVICE` | `mergus`, the keychain service of every secret |
| `SecretError` | `NotFound` (`MG-1003`): the reference and both places looked at, never a value |
| `python_environment` | The environment of the Python process: an allowlist, so no secret or cloud SDK credential (MG-REQ-1003) |
| `SecretString`, `ExposeSecret` | Re-exported from `secrecy`: the value, read only through `expose_secret` |

## Data flow

1. A caller parses `secret:<name>` with `SecretRef::parse`, which accepts only valid names.
2. It calls `resolve` with the `Environment` and `Keychain` ports of its wiring.
3. `resolve` reads the variable; a set variable answers at once, wrapped in `SecretString`.
4. Otherwise it reads the keychain entry of service `mergus` and account `<name>`; an entry
   answers, wrapped in `SecretString`.
5. Otherwise it returns `SecretError::NotFound`. A keychain that failed keeps its port error,
   which carries no value.

`origin` runs the same private chain and keeps only the `SecretOrigin` of the link that
answered; the value is dropped inside the crate and never returned.

`store` refuses an empty value, then calls `Keychain::set` with the service `mergus` and the
name. A port error becomes `StoreError::Unwritable`; nothing else is tried.

`python_environment` reads `PATH`, `LANG`, `LC_ALL`, `TZ`, `SYSTEMROOT` and `VIRTUAL_ENV` with
`Environment::var`, keeps those that are set, adds `PYTHONHASHSEED=0`, `PYTHONNOUSERSITE=1`
and `TMP`, `TEMP` and `TMPDIR` set to the work directory it is given, and returns them sorted
by name. No other variable is read.

## Dependencies

- `mergus-ports`, without the feature `real`: the `Environment` and `Keychain` traits.
- `mergus-project`: `is_valid_name`, the name rule of MG-REQ-0305.
- `secrecy`, without the feature `serde`: `SecretString`.
- `thiserror`: derives `SecretError`.
- Dev-dependencies on `mergus-sim`, for the simulated ports, on `serde`, to check that a
  value cannot be serialized, and on `mergus-ports` with the feature `real`, to run a real
  `python3` with the clean environment when one is installed.
