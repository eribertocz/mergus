# mergus-cli

The `mergus` binary; translates arguments and output only.

## What it does

- Builds the `mergus` binary and wires the real ports of `mergus-ports` into it.
- Runs `mergus validate`: every check of `mergus-project` on the nearest directory, from the
  current one upwards, that holds `project.toml`. It reports every problem in one pass and
  writes no file (MG-REQ-1344, MG-REQ-1345).
- With `--json`, prints only `mergus_events` events on standard output, one per line, ending
  with `run_finished` (MG-REQ-0260, MG-REQ-0261). Otherwise it prints plain diagnostics on
  standard error.
- Ends with an exit code of `contracts/events.md` section 2 (MG-REQ-0262). Every error has an
  `MG-####` code with a file in `errors/`, a hint, and its location when it has one
  (MG-REQ-0008, MG-REQ-1324, MG-REQ-1334).
- Defines in one place every name derived from the product name, such as the `MERGUS_`
  prefix of environment variables (MG-REQ-1325). The product name itself comes from
  `mergus_contracts::names::BINARY` (MG-REQ-0207).

## What it does not

- It has no command other than `validate` yet, no human output modes and no error layout.
- It never sets `actor` and ignores any actor given in an argument, a variable or a project
  file (MG-REQ-0264).
- It holds no business logic: it parses input, calls other crates and renders their results.
