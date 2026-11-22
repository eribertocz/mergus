# mergus-project architecture

## Main types

| Type | Role |
|---|---|
| `Layout` | The models of a project |
| `Model`, `ModelLanguage` | One model file: name, path, SQL or Python, publication |
| `Publication` | `Gold { schema, table }` or `Intermediate` |
| `ProjectError` | A problem with its `MG-####` code, message and path |
| `NameKind` | What an invalid name names: model, folder, schema, source or seed |
| `LayoutError` | A port failure, or every `ProjectError` found |

Functions: `read_layout`, `is_valid_name`, `suggest_valid_name` and `closest_name`.

## Data flow

1. The caller passes a `FileSystem` and the project root to `read_layout`.
2. It lists `models/` depth first, entries in the port's sorted order. An entry the port can
   list is a directory; any other is a file, and a model when it ends in `.sql` or `.py`.
3. Each folder, schema and model name is checked as it is met; then the stems of
   `sources/*.toml` and `seeds/*.csv`.
4. Duplicate model names are found last, the first file in walking order being the original.
5. With no problem it returns the `Layout`; otherwise every `ProjectError`, in that order.
   Paths are relative to the root and separated by `/`.

## Dependencies

- `mergus-ports`: the `FileSystem` port and `PortError`.
- `thiserror`: derives the error types.
- `mergus-sim`, as a dev-dependency only: the in-memory file system of the tests.
