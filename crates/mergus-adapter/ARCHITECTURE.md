# mergus-adapter architecture

## Main types

| Type | Role |
|---|---|
| `StorageAdapter` | The six operations every adapter implements |
| `BatchStream` | A boxed Arrow `RecordBatchReader` of one table |
| `Capabilities` | What an adapter declares; the default declares nothing |
| `Versioning`, `WriteMode` | Parts of the declaration |
| `VersionRef` | The storage's name of one data version, returned by `commit` |
| `PinRecipe` | How to read a version pinned, stored in the manifest |
| `Retention` | The live versions and the rollback window `expire` must keep |
| `Requirement`, `check_capabilities` | What a run needs, and the check of MG-REQ-0287 |
| `AdapterError`, `CapabilityError` | The failure of an operation; the undeclared needs |

## Data flow

1. A caller reads `capabilities` and lists its `Requirement`s.
2. `check_capabilities` refuses any requirement not declared, before data is extracted.
3. The caller `write`s one `BatchStream` per table, then `commit`s and gets a `VersionRef`.
4. `pin_recipe` describes that version for the manifest; `read_at` reads a table at it.
5. `expire` deletes old versions, keeping those of `Retention`.

## Dependencies

- `arrow`, without default features: record batches, readers and data types.
- `thiserror`: derives the errors.
