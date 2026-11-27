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
| `PolarsError`, `StepFailure` | `StepFailed` (`MG-0618`), `PolarsMissing` (`MG-0620`), `Child`, `Boundary` or a `PortError` |

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
              ─► status.json: ok, exception, not_lazy_frame or no_polars
  ─► status.json ─► PolarsError, or FileSystem::open ─► schema.arrow / output.arrow
```

The child runs with `-I -S`, so it starts without `site`; it calls `site.main()` itself
before anything else, so the interpreter's own packages, where `polars` is installed, are
importable. Its report goes to `status.json`, not to standard output, so what the step
prints cannot be taken for it. A child that stops without a report is `PolarsError::Child`,
with its exit code and standard error.

## Dependencies

- `mergus-ports`: the `Python` and `FileSystem` ports.
- `mergus-security`: `python_command`, the allowlisted environment of the child.
- `arrow` (feature `ipc`): writes and reads the Arrow IPC files of the step's directory.
- `serde`, `serde_json`: read the child's `status.json`.
- `thiserror`: derives `PolarsError` and `StepFailure`.
- Tests: the real ports (`real`) and `mergus-sim`'s `SimEnvironment`; they need a Python with
  `polars`.
