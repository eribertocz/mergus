# mergus-engine-polars

Execute Polars steps.

## What it does

- Runs every Polars step at the `subprocess` level, the only one in this build
  (MG-REQ-0635.3): each call is a Python child started through the `Python` port
  (`Python::run`), never through the `Process` port.
- Runs the child in the step's own directory, `<work dir>/polars/<model>/<step>`
  (`step_dir`), which it removes and creates empty before each call, so the directory is
  fresh for each step and each run (MG-REQ-0636.1).
- Gives the caller the child's whole environment (`child_environment`): the allowlisted one
  of `mergus_security::python_command`, with no secret value (MG-REQ-0006). The caller
  passes it to `RealPython::locate`.
- Imports the model file by its path, with the project's `lib` directory first on
  `sys.path`, and calls the step with one keyword argument per parameter (MG-REQ-0617.1).
- Plans a step (`plan_step`): writes a zero-row Arrow IPC file per input, calls the step once
  with `pl.LazyFrame(schema=…)` of each, and reads the output schema from the zero-row
  `schema.arrow` the child writes (MG-REQ-0617.2).
- Executes a step (`execute_step`): writes each input as an Arrow IPC file, calls the step
  once with `pl.scan_ipc(<file>)` of each, and reads the `output.arrow` the child collects
  once (MG-REQ-0617.3).
- Writes and reads every Arrow IPC file batch by batch through `FileSystem::create` and
  `FileSystem::open`.
- Fails with `MG-0618` when the step raises an exception or does not return a LazyFrame,
  naming the model, the step, and the exception's type and message or the returned type, and
  with `MG-0620` when the child cannot import `polars`, with the fix "install polars in the
  project's Python" (MG-REQ-0617.4).
- Adds nothing to the model's module: a step stays a plain function that a Python test calls
  with a LazyFrame built by hand, without Mergus (MG-REQ-0619.1).

## What it does not

- It has no `native` or `in_process` level, and no server mode: those come in later phases.
- It does not read or validate the model file, find its steps or their parameters, or plan
  segments: `mergus-project` and `mergus-plan` do.
- It does not locate the interpreter or enable the real ports: `mergus-runtime` does, with
  `RealPython::locate` and `child_environment`.
- It sets no time limit on a step.
- It prints nothing and keeps no state between calls.
