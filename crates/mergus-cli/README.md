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
- Runs `mergus ls [<selection>]`: every selected model, one per line, with why it is
  selected. Selectors are `<model>`, `<model>+`, `+<model>`, `+<model>+`, `tag:<t>`,
  `source:<s>` (models that read a table of the source), `path:<folder>` (relative to the
  project root), `changed`, a union by space, an intersection by comma and `--exclude`; no
  selection is every model. Without storage, `changed` is every model. No command asks for a
  previous manifest or state file (MG-REQ-1305, MG-REQ-1306, MG-REQ-1352.1).
- Runs `mergus plan [<selection>]`: writes the plan of `mergus-plan` to
  `.mergus/plans/<plan_id>.json`, keeps an existing file of that name, emits `plan_ready`
  and executes nothing (MG-REQ-0746).
- Runs `mergus run [<selection>]` and `mergus backfill <model> --from <date> [--to <date>]`:
  stages 1 to 4 as `plan`, where an error exits with code 1 and executes nothing, then
  stage 5 into candidates in work. `.sql` models that read other models run in DuckDB; the
  human output counts the steps computed and skipped and says nothing was published. Ctrl-C
  sets the cancel token and the run exits with code 7 (MG-REQ-0710). The limits of
  `[limits]` in `project.toml` and of `--max-wall-time` (`90s`, `30m`, `2h`),
  `--max-rows-written` and `--max-bytes-scanned` (positive integers), the smaller of each
  pair, go to the run cycle; an invalid flag value is an invalid command line (MG-REQ-0718).
- Takes `--idempotency-key <key>` on `init`, `new model`, `add source`, `secret set`, `run`
  and `backfill`: within 24 hours the same key and arguments print the first result again
  without new effects, and other arguments fail with `MG-1314` (MG-REQ-1313).
- Resolves `max_parallel`, an integer from 1 (default 4), from `--max-parallel`, then
  `MERGUS_MAX_PARALLEL`, then `max_parallel` in `<config dir>/mergus/config.toml`, the
  directory given by the `Environment` port. A missing file is empty; an unknown key is an
  error (MG-REQ-1309, MG-REQ-1351).
- Runs `mergus explain <argument>`: the catalog file of an `MG-####` code, otherwise the
  model's name, identifier, file, inputs, readers and every effective option with its value
  and origin; an unknown code or model names the closest one (MG-REQ-1310, MG-REQ-1352).
- Runs `mergus secret set <name>`: reads the value from standard input and stores it in the
  keychain under the service `mergus` and the account `<name>`. On a terminal it writes the
  prompt `Value of secret <name>: ` on standard error once and reads one line without echo;
  otherwise it reads all of standard input and removes one trailing line ending. An empty
  value (`MG-1001`) or a keychain that cannot be written (`MG-1002`) stores nothing. The
  value is never an argument, and no output shows it (MG-REQ-1007). `--dry-run` makes the
  same checks and stores nothing (MG-REQ-0192).
- Runs `mergus doctor`: one line `<name>: <origin>` per secret reference the sources use in
  `connection` or `replica`, once each, sorted by name; the origin is `environment
  MERGUS_SECRET_<NAME>`, `keychain mergus/<name>` or `missing`. No output shows a value. Each
  missing reference is an `MG-1003` error whose fix is `mergus secret set <name>`, all in one
  pass; it writes nothing and takes no `--dry-run` (MG-REQ-1004).
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

- `run` does not execute `.py` models, models that read `raw.<table>` or seeds, or
  extractions yet, and emits no step events yet.
- It has no live tree or summary of the interactive mode, and `mergus doctor` has no
  `--source`, `--security` or `--network` yet.
- It never sets `actor` and ignores any actor given in an argument, a variable or a project
  file (MG-REQ-0264).
- It holds no business logic: it parses input, calls other crates and renders their results.
