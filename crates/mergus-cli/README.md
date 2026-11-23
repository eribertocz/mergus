# mergus-cli

The `mergus` binary; translates arguments and output only.

## What it does

- Builds the `mergus` binary and wires the real ports of `mergus-ports` into it.
- Runs `mergus validate`: every check of `mergus-project` on the nearest directory, from the
  current one upwards, that holds `project.toml`. It reports every problem in one pass and
  writes no file (MG-REQ-1344, MG-REQ-1345).
- Runs `mergus init`: at most three questions (name, profile, gold location) with defaults,
  asked only when standard input is a terminal and `--yes` is absent; writes `project.toml`
  with its `#:schema` line, `models/`, `sources/` and `.gitignore`, then prints the next
  three commands (MG-REQ-0321, MG-REQ-1301, MG-REQ-1349).
- Runs `mergus new model` and `mergus add source`: one model file with a minimal SQL or
  Python example, or one source file whose connection is a secret reference. An invalid or
  taken name or an existing file writes nothing; `--dry-run` lists the files instead
  (MG-REQ-1304, MG-REQ-1350).
- Runs `mergus schema <command>` and `mergus schema --all`: the `mergus_cli` document of the
  command line, alone on standard output (MG-REQ-0263, MG-REQ-1348).
- With `--json`, prints only `mergus_events` events on standard output, one per line, ending
  with `run_finished` (MG-REQ-0260, MG-REQ-0261). Otherwise it prints every error on
  standard error, grouped by file, in the layout of MG-REQ-1322: code, message, location,
  source excerpt, `why`, `fix` and `more` (MG-REQ-1322, MG-REQ-1323, MG-REQ-1346).
- Is interactive, with colors, only when standard output is a terminal, `--json` is absent
  and `TERM` is not `dumb`; plain otherwise. A non-empty `NO_COLOR` removes colors.
  `--quiet` shows only errors, `--verbose` adds one line per stage with its duration, and the
  two together are an invalid command line (MG-REQ-1321, MG-REQ-1347).
- Ends with an exit code of `contracts/events.md` section 2 (MG-REQ-0262). Every error has an
  `MG-####` code with a file in `errors/`, a hint, and its location when it has one
  (MG-REQ-0008, MG-REQ-1324, MG-REQ-1334).
- Defines in one place every name derived from the product name, such as the `MERGUS_`
  prefix of environment variables (MG-REQ-1325). The product name itself comes from
  `mergus_contracts::names::BINARY` (MG-REQ-0207).

## What it does not

- It has no selection, settings, `explain` or `plan` yet, and no live tree or summary of the
  interactive mode.
- It never sets `actor` and ignores any actor given in an argument, a variable or a project
  file (MG-REQ-0264).
- It holds no business logic: it parses input, calls other crates and renders their results.
