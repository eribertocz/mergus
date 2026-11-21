# mergus-cli

The `mergus` binary; translates arguments and output only.

## What it does

- Builds the `mergus` binary.
- Defines in one place every name derived from the product name, such as the `MERGUS_`
  prefix of environment variables (MG-REQ-1325). The product name itself comes from
  `mergus_contracts::names::BINARY` (MG-REQ-0207).

## What it does not

- It has no commands yet: `mergus` does nothing and exits with status 0. Commands, `--json`,
  output modes and error rendering come with later issues.
- It holds no business logic: it parses input, calls other crates and renders their results.
