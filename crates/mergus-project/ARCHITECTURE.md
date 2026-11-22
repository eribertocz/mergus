# mergus-project architecture

## Main types

| Type | Role |
|---|---|
| `Layout` | The models of a project |
| `Model`, `ModelLanguage` | One model file: name, path, SQL or Python, publication |
| `Publication` | `Gold { schema, table }` or `Intermediate` |
| `ProjectError` | A problem with its `MG-####` code, message and path |
| `NameKind` | What an invalid name names: model, folder, schema, source or seed |
| `LayoutError` | A port failure, a broken bundled schema, or every `ProjectError` found |
| `Configuration` | `project.toml` and the sources |
| `ConfigFile` | One configuration file: path, JSON value, canonical JSON |
| `Source` | One source: the stem of its file, and the file |
| `ConfigKind` | Project or source, which picks the schema of `schemas/` of this crate |

Functions: `read_layout`, `read_configuration`, `is_valid_name`, `suggest_valid_name` and `closest_name`.

## Data flow

1. The caller passes a `FileSystem` and the project root to `read_layout`.
2. It lists `models/` depth first, entries in the port's sorted order. An entry the port can
   list is a directory; any other is a file, and a model when it ends in `.sql` or `.py`.
3. Each folder, schema and model name is checked as it is met; then the stems of
   `sources/*.toml` and `seeds/*.csv`.
4. Duplicate model names are found last, the first file in walking order being the original.
5. With no problem it returns the `Layout`; otherwise every `ProjectError`, in that order.
   Paths are relative to the root and separated by `/`.

## Configuration flow

1. `read_configuration` reads `project.toml`, then each `sources/*.toml` in the port's order.
2. Each file is parsed as TOML 1.0 with the line of every key and value, and mapped to JSON;
   string values with `${`, `{{` or `{%` are reported, as are NaN and infinite floats.
3. The JSON is validated against the bundled schema of its kind; unknown keys get the closest
   allowed key of their place.
4. The canonical JSON is the RFC 8785 form of `mergus-contracts`.

## Dependencies

- `mergus-ports`: the `FileSystem` port and `PortError`.
- `mergus-contracts`: `canonical_json`, the RFC 8785 serializer.
- `toml` 0.9.8, with `toml_parser` 1.0.4 in `Cargo.lock`: the last releases that parse TOML
  1.0 rather than 1.1.
- `jsonschema` 0.20.0 without default features (no file or network resolution): later
  releases depend on `borrow-or-share`, whose MIT-0 license `deny.toml` does not allow.
- `serde_json`: the JSON values.
- `thiserror`: derives the error types.
- `mergus-sim`, as a dev-dependency only: the in-memory file system of the tests.
