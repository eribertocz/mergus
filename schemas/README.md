# Contract schemas

Every contract Mergus owns has its JSON Schema here (MG-REQ-0201). Its Rust types are
generated from the schema into `crates/mergus-contracts/src/generated/` (MG-REQ-0202).

## Layout

```
schemas/
  <contract>/              the contract name, in snake_case
    v<version>/            one directory per schema version: v1, v2, ...
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

## Generating the types

After changing a schema, run `cargo xtask contracts-generate`. `cargo xtask
contracts-generated`, a step of `just check`, fails when the generated files are out of date.
Never edit a generated file by hand.
