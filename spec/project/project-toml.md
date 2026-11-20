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
