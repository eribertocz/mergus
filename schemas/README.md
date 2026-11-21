# Contract schemas

Every contract Mergus owns has its JSON Schema here (MG-REQ-0201). Its Rust types are
generated from the schema into `crates/mergus-contracts/src/generated/` (MG-REQ-0202).

## Layout

```
schemas/
  <contract>/              the contract name, in snake_case
    v<version>/            one directory per major version: v1, v2, ...
      VERSION              the full version of the contract, MAJOR.MINOR.PATCH
      schema.json          the JSON Schema 2020-12
      valid/*.json         at least one document the schema accepts
      invalid/*.json       at least one document the schema rejects
```

## Rules for a schema

- `$schema` is `https://json-schema.org/draft/2020-12/schema`.
- Every object sets `additionalProperties: false`.
- The root object requires the properties `contract` and `contract_version`.
- Every schema node has a `description`; it becomes the documentation of the generated type
  or field.
- Only these keywords are used: `$schema`, `$id`, `$defs` (root only), `title`, `description`,
  `$ref` (to `#/$defs/<name>` only), `type` (one type), `const`, `enum` (of strings),
  `properties`, `required`, `additionalProperties` and `items`. The generator and the
  validator of `mergus-contracts` refuse any other keyword, so none is silently ignored.

## Versions

Each contract has its own semantic version `MAJOR.MINOR.PATCH`, independent of the Mergus
version (MG-REQ-0204). The file `VERSION` of each `v<MAJOR>/` directory holds it on one line,
and `mergus-contracts` exports it as the constant `SEMANTIC_VERSION` of the contract type.
A document's `contract_version` stays the major version, an integer.

When you change `schema.json`, change `VERSION` with it:

- A change to `title` or `description` only is a patch change: raise `PATCH`.
- Adding an optional property or a `$defs` entry is a minor change: raise `MINOR` and set
  `PATCH` to 0.
- Any other change is major, a loosening one included: leave `v<MAJOR>/` as it is and add a
  new directory `v<MAJOR+1>/` whose `VERSION` is `<MAJOR+1>.0.0`.
- A new contract starts at `v1/` with `VERSION` `1.0.0`.

The `contract-compatibility` guard fails a pull request whose `VERSION` does not match its
change, or whose `VERSION` is missing or malformed (MG-REQ-0205). The generator refuses a
contract version without a well-formed `VERSION` file.

## Generating the types

After changing a schema, run `cargo xtask contracts-generate`. `cargo xtask
contracts-generated`, a step of `just check`, fails when the generated files are out of date.
Never edit a generated file by hand.
