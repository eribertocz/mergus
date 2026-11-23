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
| `ConfigKind` | Project, source or folder, which picks the schema of `schemas/` of this crate |
| `ModelOptions` | The effective options of one model, and its effective publication |
| `Effective` | One option: a single `Setting`, or the `Setting` of each list item |
| `Setting`, `Origin` | A JSON value and where it comes from: file and line, convention or default |
| `Identifier`, `IdKind` | A stable identifier and the kind that picks its prefix |
| `Renames` | The entries of `ids.toml`: current name to stored identifier |
| `Identifiers` | Gives each object its identifier and collects the ones given twice |

Functions: `read_layout`, `read_configuration`, `read_model_options`, `is_valid_name`, `suggest_valid_name`, `closest_name`, `read_renames`, `derive_identifier` and `read_sql_references`.

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

## Folder options flow

1. `read_model_options` takes the `Layout` and walks `models/` and its folders, sorted, reading
   each `mergus.toml` like the other configuration files.
2. Top-level keys other than `all` that name no model of the folder are reported and removed;
   the rest is validated against `folder.schema.json`. Problems of a file are sorted by line,
   code and key.
3. With no problem, each model gets its levels, most specific first: its section, `[all]` of
   its folder, then `[all]` of each parent up to `models/`, unless `inherit = false`.
4. Each option of the phase takes the first level that sets it, else the convention
   (`publish`) or its default; `checks` and `tags` take every level's items, first one wins.
   `null` stands for an option with no value.

## Identifiers flow

1. `read_renames` reads `ids.toml` if present: only `[model]` and `[source]`, each value an
   identifier of its table's prefix, no identifier twice. Every value is reserved.
2. The caller creates `Identifiers` with the `Renames` and asks for each object in a fixed
   order: models and sources by name, source tables under their source's identifier, columns
   under their model's or table's identifier.
3. A model or source with an entry takes it; any other object takes the SHA-256 derivation of
   MG-REQ-0369 with the first `<n>` from 0 whose result is not reserved.
4. `finish` returns every identifier given to a second object, naming both and their files.

## Dependencies

- `mergus-ports`: the `FileSystem` port and `PortError`.
- `mergus-contracts`: `canonical_json`, the RFC 8785 serializer, and `hash::sha256`, the
  SHA-256 of derived identifiers behind a Mergus interface (MG-REQ-0186).
- `toml` 0.9.8, with `toml_parser` 1.0.4 in `Cargo.lock`: the last releases that parse TOML
  1.0 rather than 1.1.
- `jsonschema` 0.63 without default features (no file or network resolution).
- `serde_json`: the JSON values.
- `thiserror`: derives the error types.
- Base32 is written in this crate and tested against the vectors of RFC 4648.
- `mergus-sim`, as a dev-dependency only: the in-memory file system of the tests.
