# mergus-engine-polars architecture

## Main types

| Item | Role |
|---|---|
| `Step` | A step: its model, the function's name, the model file and the project's `lib` |
| `Parameter` | A parameter's name and its input's schema, for planning |
| `Frame` | A parameter's name, its input's schema and rows, for execution |
| `Output` | The collected result of a step: schema and record batches |
| `plan_step` | Calls a step with empty LazyFrames and returns its output schema (MG-REQ-0617.2) |
| `execute_step` | Calls a step with `pl.scan_ipc` inputs and returns its result (MG-REQ-0617.3) |
| `step_dir` | The working directory of a step under the run's work directory |
| `child_environment` | The allowlisted environment of the Python child (MG-REQ-0006) |
| `PolarsError`, `StepFailure` | `StepFailed` (`MG-0627`), `LoadFailed` (`MG-0628`), `PolarsMissing` (`MG-0629`), `Child`, `InvalidName`, `Boundary` or a `PortError` |

## Data flow

```
plan_step / execute_step
  ─► step_dir: FileSystem::remove_dir_all if it exists, then create_dir_all   (MG-REQ-0636.1)
  ─► each input: FileSystem::create ─► Arrow IPC input-<n>.arrow
       (zero rows for planning, the input's rows for execution)
  ─► Python::run(src/child.py, [mode, model file, lib, step, (name, file)...], step_dir)
       child: site.main() ─► lib first on sys.path ─► import polars
              ─► import the model file by its path ─► step(**frames)
                   plan:    pl.LazyFrame(schema=pl.read_ipc_schema(file))
                   execute: pl.scan_ipc(file)
              ─► plan: zero-row schema.arrow    execute: collect() once ─► output.arrow
              ─► status.json: ok, exception, not_lazy_frame, load_failed or no_polars
  ─► status.json ─► PolarsError, or FileSystem::open ─► schema.arrow / output.arrow
```

The child runs with `-I -S`, so it starts without `site`; it calls `site.main()` itself
before anything else, so the interpreter's own packages, where `polars` is installed, are
importable. Its report goes to `status.json`, not to standard output, so what the step
prints cannot be taken for it. A child that stops without a report is `PolarsError::Child`,
with its exit code and standard error.

The child is registered in `sys.modules` under `__mergus_model__`, a key no import can name,
so a model file named like a module of the standard library or of `lib` never shadows it.
Every exception, `BaseException` included (`SystemExit`, `KeyboardInterrupt`), becomes a
report. The child records its working directory first and writes every file by absolute
path, so a step that changes the current directory keeps its report.

`step_dir` accepts a model or step name only when it is exactly one plain path component,
and `fresh_dir` removes only a directory under the run's work directory.

## Limits and caller duties

- `RealPython` carries the child's environment from `RealPython::locate`, including `TMP`,
  `TEMP` and `TMPDIR` set to the run's work directory. The caller builds one `RealPython`
  per run, with `child_environment` for that run's work directory.
- Until #265 adds limits, nothing bounds the child: `Python::run` has no time limit, stdout
  and stderr are read whole into memory, and a grandchild that keeps a pipe open keeps the
  call waiting.

## Dependencies

- `mergus-ports`: the `Python` and `FileSystem` ports.
- `mergus-security`: `python_command`, the allowlisted environment of the child.
- `arrow` (feature `ipc`): writes and reads the Arrow IPC files of the step's directory.
- `serde`, `serde_json`: read the child's `status.json`.
- `thiserror`: derives `PolarsError` and `StepFailure`.
- Tests: the real ports (`real`), `mergus-sim`'s `SimEnvironment` and `SimPython`; most need
  a Python with `polars`, and the contract tests use a fake child instead.
