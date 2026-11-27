# The Python child of a Polars step at the `subprocess` level (MG-REQ-0617).
#
# Arguments: mode (`plan` or `execute`), the model file, the project's `lib` directory, the
# step, then one parameter name and Arrow IPC file per input. It runs in the step's working
# directory, writes `schema.arrow` (plan) or `output.arrow` (execute) there, and always ends by
# writing `status.json`, which says how the step went.
import json
import site
import sys

# `-S` kept `site` out; the interpreter's own packages, where `polars` is, are added here.
site.main()


def report(status):
    with open("status.json", "w", encoding="utf-8") as file:
        json.dump(status, file)


def main():
    mode, module_path, lib, step = sys.argv[1:5]
    pairs = sys.argv[5:]
    inputs = list(zip(pairs[0::2], pairs[1::2]))
    sys.path.insert(0, lib)
    try:
        import polars as pl
    except ImportError as error:
        report({"status": "no_polars", "message": str(error)})
        return
    import importlib.util
    import os

    try:
        name = os.path.splitext(os.path.basename(module_path))[0]
        spec = importlib.util.spec_from_file_location(name, module_path)
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
        function = getattr(module, step)
        if mode == "plan":
            frames = {p: pl.LazyFrame(schema=pl.read_ipc_schema(f)) for p, f in inputs}
        else:
            frames = {p: pl.scan_ipc(f) for p, f in inputs}
        result = function(**frames)
        if not isinstance(result, pl.LazyFrame):
            report({"status": "not_lazy_frame", "returned": type(result).__name__})
            return
        if mode == "plan":
            empty = pl.DataFrame(schema=result.collect_schema())
            empty.write_ipc("schema.arrow", compression="uncompressed")
        else:
            result.collect().write_ipc("output.arrow", compression="uncompressed")
    except Exception as error:
        report({"status": "exception", "type": type(error).__name__, "message": str(error)})
        return
    report({"status": "ok"})


main()
