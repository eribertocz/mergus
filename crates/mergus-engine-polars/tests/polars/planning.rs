//! MG-REQ-0617: the step is called once with empty LazyFrames for its plan and schema, and
//! once with `pl.scan_ipc` inputs for its result; its failures are typed.

use mergus_engine_polars::{
    Frame, Parameter, PolarsError, Step, StepFailure, execute_step, plan_step, step_dir,
};
use std::path::Path;

use mergus_ports::real::{RealFileSystem, RealPython};

use super::{batch, column, floats, python, read, scratch, step, write};

const MODEL: &str = r#"import polars as pl
from helpers import usd

def in_usd(*, joined):
    import this
    open("lib_first", "w").write(this.SOURCE)
    open("rows_seen", "w").write(str(joined.collect().height))
    return joined.with_columns(usd(pl.col("amount"), pl.col("rate")).alias("amount_usd"))

def fails(joined):
    raise ValueError("no rate for 2026-01-01")

def not_lazy(joined):
    return "SELECT 1"

def bad_column(joined):
    return joined.select(pl.col("no_such_column"))
"#;

#[test]
fn mg_req_0617_polars_planned_with_empty_frames() {
    let root = scratch("planning");
    write(&root.join("models").join("model.py"), MODEL);
    write(
        &root.join("lib").join("helpers.py"),
        "def usd(a, r):\n    return a * r\n",
    );
    // `this` is also a module of the standard library: the project's `lib` comes first.
    write(&root.join("lib").join("this.py"), "SOURCE = 'lib'\n");
    let work = root.join("work");
    let Some(python) = python(&root, &work) else {
        return;
    };
    let joined = floats(&["amount", "rate"]);
    let parameters = [Parameter {
        name: "joined".to_owned(),
        schema: joined.clone(),
    }];
    let inputs = [Frame {
        name: "joined".to_owned(),
        schema: joined.clone(),
        batches: vec![batch(&joined, &[&[10.0, 2.0], &[2.0, 0.5]])],
    }];
    plans_and_executes(&python, &root, &work, &parameters, &inputs);
    failures_are_typed(&python, &root, &work, &parameters, &inputs);
    load_failures_are_typed(&python, &root, &work, &parameters);
    missing_polars_is_typed(&python, &root, &work, &parameters);
}

/// Planning calls the step once with an empty LazyFrame of the input's schema; execution
/// passes the input as `pl.scan_ipc` and collects the result once.
#[cfg(test)]
fn plans_and_executes(
    python: &RealPython,
    root: &Path,
    work: &Path,
    parameters: &[Parameter],
    inputs: &[Frame],
) {
    let in_usd = step(root, "sales", "in_usd");
    let dir = step_dir(work, &in_usd);
    let schema = plan_step(python, &RealFileSystem, &in_usd, parameters, work).unwrap();
    assert_eq!(*schema, *floats(&["amount", "rate", "amount_usd"]));
    assert_eq!(read(&dir.join("rows_seen")), "0");
    assert_eq!(read(&dir.join("lib_first")), "lib");
    let output = execute_step(python, &RealFileSystem, &in_usd, inputs, work).unwrap();
    assert_eq!(*output.schema, *schema);
    assert_eq!(column(&output.batches, "amount_usd"), [20.0, 1.0]);
    assert_eq!(read(&dir.join("rows_seen")), "2");
}

/// An exception in the call, a return value that is not a LazyFrame, and an exception while
/// the result is collected are `MG-0618`.
#[cfg(test)]
fn failures_are_typed(
    python: &RealPython,
    root: &Path,
    work: &Path,
    parameters: &[Parameter],
    inputs: &[Frame],
) {
    let fails = step(root, "sales", "fails");
    let error = plan_step(python, &RealFileSystem, &fails, parameters, work).unwrap_err();
    assert_eq!(error.code(), Some("MG-0618"));
    assert!(
        matches!(
            &error,
            PolarsError::StepFailed { model, step, failure: StepFailure::Exception { kind, message } }
                if model == "sales" && step == "fails" && kind == "ValueError"
                    && message == "no rate for 2026-01-01"
        ),
        "{error:?}"
    );
    let not_lazy = step(root, "sales", "not_lazy");
    let error = execute_step(python, &RealFileSystem, &not_lazy, inputs, work).unwrap_err();
    assert_eq!(error.code(), Some("MG-0618"));
    assert!(
        matches!(
            &error,
            PolarsError::StepFailed { failure: StepFailure::NotLazyFrame { returned }, .. }
                if returned == "str"
        ),
        "{error:?}"
    );
    let bad_column = step(root, "sales", "bad_column");
    let error = execute_step(python, &RealFileSystem, &bad_column, inputs, work).unwrap_err();
    assert_eq!(error.code(), Some("MG-0618"));
    assert!(
        matches!(&error, PolarsError::StepFailed { failure: StepFailure::Exception { kind, .. }, .. }
            if kind == "ColumnNotFoundError"),
        "{error:?}"
    );
}

/// A model file that raises when imported, or that lacks the step's function, did not load:
/// `MG-0619`, naming the model and the file.
#[cfg(test)]
fn load_failures_are_typed(
    python: &RealPython,
    root: &Path,
    work: &Path,
    parameters: &[Parameter],
) {
    let missing = step(root, "sales", "no_such_step");
    let error = plan_step(python, &RealFileSystem, &missing, parameters, work).unwrap_err();
    assert_eq!(error.code(), Some("MG-0619"));
    assert!(
        matches!(&error, PolarsError::LoadFailed { model, file, kind, .. }
            if model == "sales" && *file == missing.module.display().to_string()
                && kind == "AttributeError"),
        "{error:?}"
    );
    let broken_file = root.join("models").join("broken.py");
    write(
        &broken_file,
        "import polars as pl\nraise RuntimeError('broken at import')\n",
    );
    let broken = Step {
        module: broken_file,
        ..step(root, "broken", "broken")
    };
    let error = plan_step(python, &RealFileSystem, &broken, parameters, work).unwrap_err();
    assert_eq!(error.code(), Some("MG-0619"));
    assert!(
        matches!(&error, PolarsError::LoadFailed { kind, message, .. }
            if kind == "RuntimeError" && message == "broken at import"),
        "{error:?}"
    );
    assert!(error.to_string().contains("fix the file"));
}

/// A child that cannot import `polars` is `MG-0620`, with its fix.
#[cfg(test)]
fn missing_polars_is_typed(
    python: &RealPython,
    root: &Path,
    work: &Path,
    parameters: &[Parameter],
) {
    write(
        &root.join("lib").join("polars.py"),
        "raise ImportError('no polars here')\n",
    );
    let in_usd = step(root, "sales", "in_usd");
    let error = plan_step(python, &RealFileSystem, &in_usd, parameters, work).unwrap_err();
    assert_eq!(error.code(), Some("MG-0620"));
    assert!(
        matches!(&error, PolarsError::PolarsMissing { message, .. } if message == "no polars here"),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("install polars in the project's Python")
    );
}
