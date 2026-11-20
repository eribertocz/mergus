---
id: project-tools
title: Commands that edit and explain the project for the user
crate: mergus-cli
phase: "0.8"
status: draft
depends_on: [project-folder-config]
---

# Project tools

The user never needs to remember where an option lives.

### MG-REQ-0351 · explain

`mergus explain <model>` MUST print every effective option of the model with its value and
origin (MG-REQ-0335), its inputs, its published name and its engine per step.

- Kind: behavior
- Verify: test `mg_req_0351_explain_prints_values_and_origins`

### MG-REQ-0352 · config

`mergus config <model> <option>=<value>` MUST write the option in the model's section of the
`mergus.toml` of the model's folder, creating file or section if needed, preserving comments
and order, and MUST validate the result before saving.

- Kind: behavior
- Verify: test `mg_req_0352_config_writes_right_file_preserving_comments`

### MG-REQ-0353 · suggest

`mergus suggest <model>` MUST propose options from deterministic rules over the model and
source metadata (size, time columns, primary keys). With `--apply` it writes them through
`config`. It MUST NOT use AI or the network beyond the configured sources.

- Kind: behavior
- Verify: test `mg_req_0353_suggest_is_deterministic`

### MG-REQ-0354 · mv

`mergus mv <old> <new>` MUST move or rename a model file and its configuration section
together, and update `contracts/<model>.json` if present.

- Kind: behavior
- Verify: test `mg_req_0354_mv_moves_model_and_configuration`

### MG-REQ-0355 · fmt

`mergus fmt` MUST rewrite every TOML file of the project in canonical order and spacing
without changing its meaning. `mergus fmt --check` exits 1 when a file is not formatted.

- Kind: behavior
- Verify: test `mg_req_0355_fmt_preserves_meaning`

### MG-REQ-0356 · contract lock

`mergus contract lock <model>` MUST write `contracts/<model>.json` with the columns, types and
nullability of the model's real output.

- Kind: behavior
- Verify: test `mg_req_0356_contract_lock_writes_real_schema`

### MG-REQ-0357 · ls --config

`mergus ls --config` MUST list every model with its published name, refresh mode, tests count
and owner.

- Kind: behavior
- Verify: test `mg_req_0357_ls_config_lists_models`

### MG-REQ-0358 · Impact before effect

When a configuration change alters a model's publication, refresh mode or published name,
`mergus plan` MUST show it before execution, classified as no data effect, compatible,
possibly incompatible, or affecting history.

- Kind: behavior
- Verify: test `mg_req_0358_plan_classifies_configuration_impact`
