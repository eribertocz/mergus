//! MG-REQ-0617: the step is called once with empty LazyFrames for its plan and schema, and
//! once with `pl.scan_ipc` inputs for its result; its failures are typed.

use mergus_engine_polars::{
    Frame, Parameter, PolarsError, StepFailure, execute_step, plan_step, step_dir,
};
use mergus_ports::real::RealFileSystem;

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
    let in_usd = step(&root, "sales", "in_usd");
    let dir = step_dir(&work, &in_usd);

    // Planning: one call with an empty LazyFrame of the input's schema.
    let schema = plan_step(&python, &RealFileSystem, &in_usd, &parameters, &work).unwrap();
    assert_eq!(*schema, *floats(&["amount", "rate", "amount_usd"]));
    assert_eq!(read(&dir.join("rows_seen")), "0");
    assert_eq!(read(&dir.join("lib_first")), "lib");

    // Execution: the input as `pl.scan_ipc`, the result collected once.
    let inputs = [Frame {
        name: "joined".to_owned(),
        schema: joined.clone(),
        batches: vec![batch(&joined, &[&[10.0, 2.0], &[2.0, 0.5]])],
    }];
    let output = execute_step(&python, &RealFileSystem, &in_usd, &inputs, &work).unwrap();
    assert_eq!(*output.schema, *schema);
    assert_eq!(column(&output.batches, "amount_usd"), [20.0, 1.0]);
    assert_eq!(read(&dir.join("rows_seen")), "2");

    // An exception, and a return value that is not a LazyFrame: MG-0618.
    let fails = plan_step(
        &python,
        &RealFileSystem,
        &step(&root, "sales", "fails"),
        &parameters,
        &work,
    );
    let error = fails.unwrap_err();
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
    let not_lazy = execute_step(
        &python,
        &RealFileSystem,
        &step(&root, "sales", "not_lazy"),
        &inputs,
        &work,
    );
    let error = not_lazy.unwrap_err();
    assert_eq!(error.code(), Some("MG-0618"));
    assert!(
        matches!(
            &error,
            PolarsError::StepFailed { failure: StepFailure::NotLazyFrame { returned }, .. }
                if returned == "str"
        ),
        "{error:?}"
    );

    // A child that cannot import `polars`: MG-0619, with its fix.
    write(
        &root.join("lib").join("polars.py"),
        "raise ImportError('no polars here')\n",
    );
    let missing = plan_step(&python, &RealFileSystem, &in_usd, &parameters, &work);
    let error = missing.unwrap_err();
    assert_eq!(error.code(), Some("MG-0619"));
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
