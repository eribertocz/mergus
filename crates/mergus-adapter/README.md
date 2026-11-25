# mergus-adapter

Storage adapter contract (public API for third parties).

## What it does

- Defines the `StorageAdapter` trait with exactly six operations (MG-REQ-0285):
  `capabilities`, `write`, `commit`, `pin_recipe`, `read_at` and `expire`.
- Moves data as Arrow record batch streams (`BatchStream`).
- Declares capabilities: versioning (native, by files, none), multi-table transactions,
  concurrent clients, exactly stored Arrow types and write modes (append, replace, replace
  period, merge by key).
- `check_capabilities` compares what a run needs with the declaration and returns a
  `CapabilityError` naming every undeclared write mode or feature. It reads only the
  declaration, so a caller runs it before any data is extracted (MG-REQ-0287).

## What it does not

- It implements no storage: the DuckLake adapter lives in `mergus-adapter-ducklake`.
- It does not map a `CapabilityError` to an exit code; that comes with the run.
- It does not define the parameters of the replace-period and merge-by-key modes yet.
