# mergus-source architecture

## Main types

| Item | Role |
|---|---|
| `check_sources` | Every problem of the sources of a `Configuration` |
| `SourceProblem` | One problem: code, severity, file, line, key and message |
| `Severity` | `Error`, which fails validation, or `Warning` |
| `SourceKind` | `database`, `files` or `rest` (MG-REQ-0401) |
| `Mode` | `copy`, `connect` or `live`, and the engines each allows (MG-REQ-0409) |

## Data flow

1. The caller reads the project with `mergus_project::read_configuration`; only a valid
   configuration is checked, so every value has the type of its schema.
2. `check_sources` takes each source in the order of the configuration and reads its file
   again through the `FileSystem` port, only to find the line of each key.
3. For a source of a known kind it checks the `engine` against the `mode` (default `copy`),
   then each table that declares `incremental`: a missing `key` is an error, a missing
   `deletes` a warning. The line is that of `engine`, of `mode` when no engine is declared,
   or of `incremental`.
4. The problems of a source are sorted by line, code and key, so their order does not
   depend on the order of the keys in memory (MG-REQ-0005).

## Dependencies

- `mergus-ports`: the `FileSystem` port and `PortError`.
- `mergus-project`: `Configuration` and `Source`, the validated source files.
- `serde_json`: the JSON values of those files.
- `toml` 0.9.8, the release `mergus-project` uses: parses a source file again only for the
  spans of its keys.
- `thiserror`: derives `SourceProblem`.
- `mergus-sim`, as a dev-dependency only: the in-memory file system of the tests.
