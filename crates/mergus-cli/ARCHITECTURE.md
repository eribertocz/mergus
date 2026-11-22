# mergus-cli architecture

## Main types

| Item | Role |
|---|---|
| `name::PRODUCT` | The product name, taken from `mergus_contracts::names::BINARY` |
| `name::env_prefix` | The `MERGUS_` prefix of environment variables, derived from `PRODUCT` |

## Data flow

1. `main` starts and returns; it exits with status 0.
2. Code that shows or reads a name derived from the product name (help, completions,
   messages, environment variables) takes it from the `name` module, never from a literal
   (MG-REQ-1325).

The `[[bin]] name` in `Cargo.toml` is the only other place that spells the product name:
Cargo cannot read a Rust constant. A unit test checks that it equals `name::PRODUCT`.

## Dependencies

- `mergus-contracts`: the product name, `names::BINARY`.
