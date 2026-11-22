---
id: project-project-toml
title: The project.toml file
crate: mergus-project
phase: "0.2"
status: draft
depends_on: [project-layout]
---

# project.toml

`project.toml` describes the project, never what a model does.

```toml
#:schema https://mergus.dev/schemas/project.schema.json
name = "sales"
mergus = ">=1.4, <2"

[gold]
profile = "cloud"
location = "s3://my-company/gold"

[environments.prod]
# see environments/ area

[seeds.countries]
columns = { code = "text", name = "text" }
```

| Table or key | Purpose | Defined in |
|---|---|---|
| `name` | Project name | here |
| `mergus` | Version requirement of the binary | here |
| `[gold]`, `[raw]`, `[ops]` | Storage profile and location of each zone | `storage/` |
| `[environments.<name>]` | Environments | `environments/` |
| `[triggers]` | Schedules and events | `run/` |
| `[security]` | PII keys, identities, secret backends | `security/` |
| `[seeds.<name>]` | Seed schemas | `model/reuse.md` |
| `[recovery]` | `rpo`, `rto`, backups | `ops/recovery.md` |
| `[resources]` | Memory, threads, temporary disk | `ops/resources.md` |
| `strict_integrity` | Failed Menura integrity checks block publication | `quality/tests.md` |

### MG-REQ-0316 · TOML validated by schema

`project.toml`, every folder `mergus.toml` and every `sources/*.toml` MUST be TOML 1.0 and
valid against their published JSON Schemas. An unknown key, a wrong type or an invalid value
MUST be an error with file, line, and the closest valid key or value.

- Kind: behavior
- Verify: test `mg_req_0316_unknown_key_rejected_with_suggestion`

### MG-REQ-0317 · Literal values only

Configuration values MUST be literals. Variables, expressions and includes MUST NOT exist.
Values that change per run are passed as run parameters (`$run_date`).

- Kind: behavior
- Verify: test `mg_req_0317_configuration_has_no_expressions`

### MG-REQ-0318 · Canonical form

Mergus MUST normalize every configuration file to canonical JSON (RFC 8785). Fingerprints
and versions are computed from the canonical form, so formatting changes never change a
fingerprint.

- Kind: behavior
- Verify: test `mg_req_0318_formatting_does_not_change_fingerprint`

### MG-REQ-0319 · Version pin

When `mergus` is set and the running binary is outside the range, Mergus MUST download and
run a version that satisfies it, as described in `distribution/`. With `--offline` and no
cached version, it MUST stop with exit code 1 before doing any work.

- Kind: behavior
- Verify: test `mg_req_0319_out_of_range_binary_switches_or_stops`
- Phase: 0.9
- Decision: 0018

### MG-REQ-0320 · Secrets only by reference

A literal secret in any configuration file (a URL with a password, a token, a private key)
MUST be a validation error. Secrets are written as `secret:<name>` (`security/`).

- Kind: behavior
- Verify: test `mg_req_0320_literal_secret_rejected`

### MG-REQ-0321 · Schema declared on the first line

`mergus init` and `mergus fmt` MUST write the `#:schema` line at the top of every TOML file
they create or format, so that editors validate and complete.

- Kind: behavior
- Verify: test `mg_req_0321_schema_line_written`

## Configuration schemas

| File | Schema | `$id` |
|---|---|---|
| `project.toml` | `schemas/config/project.schema.json` | `https://mergus.dev/schemas/project.schema.json` |
| folder `mergus.toml` | `schemas/config/folder.schema.json` | `https://mergus.dev/schemas/folder.schema.json` |
| `sources/*.toml` | `schemas/config/source.schema.json` | `https://mergus.dev/schemas/source.schema.json` |

### MG-REQ-0361 · Configuration schema files

Each configuration schema MUST be a JSON Schema 2020-12 file at the path of the table above,
with that `$id`, closed objects (`additionalProperties: false`) and a `description` on every
node. It MAY use, besides the keywords of contract schemas, `type` as an array of types,
`oneOf`, `pattern`, `minimum`, `maximum` and `default`, and nothing else. It has no `contract`
or `contract_version` property: configuration files are not contract documents.

- Kind: behavior
- Verify: test `mg_req_0361_configuration_schemas_are_closed`
- Decision: 0018

### MG-REQ-0362 · Keys known in phase 0.2

A key MUST be in a configuration schema only once the rule that gives it meaning is due. In
phase 0.2 the schemas hold exactly:

1. MG-REQ-0362.1: `project.toml`: `name` (required, a name of MG-REQ-0305), `mergus` (a
   version requirement such as `">=1.4, <2"`), and the tables `[gold]`, `[raw]` and `[ops]`,
   each with `profile` and `location`, both strings;
2. MG-REQ-0362.2: folder `mergus.toml`: the table `[all]` and one table per model, each with
   the options of `project/folder-config.md` section 1;
3. MG-REQ-0362.3: `sources/*.toml`: no key yet; a source file is valid TOML and nothing else
   is checked until the rules of `extract/sources.md` are due.

Each later phase adds its keys as a minor version of the schema (MG-REQ-0204).

- Kind: behavior
- Verify: test `mg_req_0362_only_due_keys_are_known`
- Decision: 0018

### MG-REQ-0363 · What a literal secret is

For MG-REQ-0320, a string value is a literal secret when it does not start with `secret:` and:

1. MG-REQ-0363.1: it is a URL with a password in its user information (`<scheme>://<user>:<password>@`);
2. MG-REQ-0363.2: it contains `-----BEGIN ` followed, on the same line, by `PRIVATE KEY-----`;
3. MG-REQ-0363.3: or its key ends with `password`, `passwd`, `secret`, `token`, `api_key`,
   `apikey` or `private_key`, compared without case, and the value is not empty.

The error names the file, the line and the key, never the value, ends with exit code 6, and
its hint shows the `secret:<name>` form.

- Kind: behavior
- Verify: test `mg_req_0363_literal_secrets_detected`
- Depends: MG-REQ-0320
- Decision: 0018

### MG-REQ-0364 · Expressions are rejected

For MG-REQ-0317, a string value that contains `${`, `{{` or `{%` MUST be an error naming the
file, line and key, with exit code 1. A key named `include` is an unknown key (MG-REQ-0316).

- Kind: behavior
- Verify: test `mg_req_0364_expressions_rejected`
- Depends: MG-REQ-0317
- Decision: 0018

### MG-REQ-0365 · TOML to canonical JSON

For MG-REQ-0318, a TOML document MUST map to JSON as: table to object, array to array,
string to string, boolean to boolean, integer to an integer number, float to a number (NaN and
infinities are an error with exit code 1), and each date or time to a string in the RFC 3339
form of TOML 1.0: `T` as separator, seconds always written, fractional seconds without
trailing zeros, `Z` for a zero offset. Comments and formatting are dropped. The result is then
serialized as RFC 8785.

- Kind: behavior
- Verify: test `mg_req_0365_toml_maps_to_canonical_json`
- Depends: MG-REQ-0318
- Decision: 0018
