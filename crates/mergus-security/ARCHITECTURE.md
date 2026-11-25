# mergus-security architecture

## Main types

| Type | Role |
|---|---|
| `SecretRef` | A reference `secret:<name>`; gives the name and the variable `MERGUS_SECRET_<NAME>` |
| `resolve` | The resolution chain of MG-REQ-1001: variable, then keychain |
| `KEYCHAIN_SERVICE` | `mergus`, the keychain service of every secret |
| `SecretError` | `NotFound`: the reference and both places looked at, never a value |
| `SecretString`, `ExposeSecret` | Re-exported from `secrecy`: the value, read only through `expose_secret` |

## Data flow

1. A caller parses `secret:<name>` with `SecretRef::parse`, which accepts only valid names.
2. It calls `resolve` with the `Environment` and `Keychain` ports of its wiring.
3. `resolve` reads the variable; a set variable answers at once, wrapped in `SecretString`.
4. Otherwise it reads the keychain entry of service `mergus` and account `<name>`; an entry
   answers, wrapped in `SecretString`.
5. Otherwise it returns `SecretError::NotFound`. A keychain that failed keeps its port error,
   which carries no value.

## Dependencies

- `mergus-ports`, without the feature `real`: the `Environment` and `Keychain` traits.
- `mergus-project`: `is_valid_name`, the name rule of MG-REQ-0305.
- `secrecy`, without the feature `serde`: `SecretString`.
- `thiserror`: derives `SecretError`.
- Dev-dependencies on `mergus-sim`, for the simulated ports, and on `serde`, to check that a
  value cannot be serialized.
