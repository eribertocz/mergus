# The Python child of a Polars step at the `subprocess` level (MG-REQ-0617).
#
# Arguments: mode (`plan` or `execute`), the model file, the project's `lib` directory, the
# step, then one parameter name and Arrow IPC file per input. It runs in the step's working
# directory, writes `schema.arrow` (plan) or `output.arrow` (execute) there, and always ends by
# writing `status.json`, which says how the step went. Every file is written by its absolute
# path, so a step that changes the current directory cannot move them.
import json
import os
import site
import sys

# `-S` kept `site` out; the interpreter's own packages, where `polars` is, are added here.
site.main()

# The working directory, recorded before any user code runs.
HERE = os.getcwd()

# The key the model is registered under in `sys.modules`: no import can name it, so the model
# never shadows a module of the standard library, of `lib` or of the interpreter.
MODEL_KEY = "__mergus_model__"


def path(name):
    return os.path.join(HERE, name)


def report(status):
    with open(path("status.json"), "w", encoding="utf-8") as file:
        json.dump(status, file)


def failure(status, error):
    return {"status": status, "type": type(error).__name__, "message": str(error)}


def load(module_path, step):
    import importlib.util

    spec = importlib.util.spec_from_file_location(MODEL_KEY, module_path)
    module = importlib.util.module_from_spec(spec)
    sys.modules.setdefault(MODEL_KEY, module)
    spec.loader.exec_module(module)
    return getattr(module, step)


def call(pl, function, mode, inputs):
    if mode == "plan":
        frames = {p: pl.LazyFrame(schema=pl.read_ipc_schema(f)) for p, f in inputs}
    else:
        frames = {p: pl.scan_ipc(f) for p, f in inputs}
    result = function(**frames)
    if not isinstance(result, pl.LazyFrame):
        return {"status": "not_lazy_frame", "returned": type(result).__name__}
    if mode == "plan":
        empty = pl.DataFrame(schema=result.collect_schema())
        empty.write_ipc(path("schema.arrow"), compression="uncompressed")
    else:
        result.collect().write_ipc(path("output.arrow"), compression="uncompressed")
    return {"status": "ok"}


def main():
    mode, module_path, lib, step = sys.argv[1:5]
    pairs = sys.argv[5:]
    inputs = list(zip(pairs[0::2], pairs[1::2]))
    sys.path.insert(0, lib)
    try:
        import polars as pl
    except BaseException as error:
        report(failure("no_polars", error))
        return
    try:
        function = load(module_path, step)
    except BaseException as error:
        report(failure("load_failed", error))
        return
    try:
        report(call(pl, function, mode, inputs))
    except BaseException as error:
        report(failure("exception", error))


main()
