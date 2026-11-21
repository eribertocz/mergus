# mergus-contracts

Rust types generated from `schemas/`.

## What it does

- Holds, in `src/generated/`, the Rust types of every contract Mergus owns, generated from
  its JSON Schema in `schemas/` by `cargo xtask contracts-generate` (MG-REQ-0202), and the
  registry `generated::CONTRACTS` of their schemas.
- Validates a contract document against its schema before it can be emitted: `validate`
  returns a `ValidDocument`, the only form code may write or print (MG-REQ-0203).
- Defines, in `names`, every name Mergus uses from another module (MG-REQ-0207).

## What it does not

- It performs no I/O: callers write the `ValidDocument` through a port (MG-REQ-0004).
- It holds no hand-written contract type (MG-REQ-0202).
- It supports only the JSON Schema keywords listed in `schemas/README.md`; a schema that uses
  another keyword is refused rather than half-checked.
- It does not map an `EmitError` to a user-facing error code; the caller ends the run with
  an internal error.
