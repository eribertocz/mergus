//! MG-REQ-0617 and MG-REQ-0619.1: the child calls a step exactly once and collects it exactly
//! once, and a model never shadows another module, whatever its file is named.

use std::path::Path;

use mergus_engine_polars::{
    Frame, Parameter, PolarsError, Step, StepFailure, execute_step, plan_step, step_dir,
};
use mergus_ports::FileSystem;
use mergus_ports::real::{RealFileSystem, RealPython};

use super::{batch, column, floats, python, read, scratch, write};

const COUNTED: &str = r#"import polars as pl

def counted(joined):
    with open("calls", "a", newline="\n") as file:
        file.write("call\n")

    def mark(series):
        with open("collects", "a", newline="\n") as file:
            file.write("collect\n")
        return series

    return joined.with_columns(pl.col("amount").map_batches(mark, return_dtype=pl.Float64))
"#;

/// A model named like a module of the standard library, which imports that module.
const DATETIME: &str = r#"from datetime import date

def datetime(joined):
    assert date(2026, 1, 1).year == 2026
    return joined
"#;

/// A model named like a module of `lib`, which imports that module.
const HELPERS: &str = r#"from helpers import usd

def helpers(joined):
    return joined.with_columns(usd)
"#;

/// The step `name` of the model file `models/<file>.py`.
#[cfg(test)]
fn step_of(root: &Path, file: &str, name: &str) -> Step {
    Step {
        model: file.to_owned(),
        step: name.to_owned(),
        module: root.join("models").join(format!("{file}.py")),
        lib: root.join("lib"),
    }
}

/// The parameter `joined` and its input of two rows.
#[cfg(test)]
fn joined() -> (Vec<Parameter>, Vec<Frame>) {
    let schema = floats(&["amount", "rate"]);
    let parameter = Parameter {
        name: "joined".to_owned(),
        schema: schema.clone(),
    };
    let frame = Frame {
        name: "joined".to_owned(),
        schema: schema.clone(),
        batches: vec![batch(&schema, &[&[10.0, 2.0], &[2.0, 0.5]])],
    };
    (vec![parameter], vec![frame])
}

#[test]
fn mg_req_0617_step_called_once_and_collected_once() {
    let root = scratch("counted");
    write(&root.join("models").join("counted.py"), COUNTED);
    let work = root.join("work");
    let Some(python) = python(&root, &work) else {
        return;
    };
    let (parameters, inputs) = joined();
    let counted = step_of(&root, "counted", "counted");
    let dir = step_dir(&work, &counted).unwrap();
    plan_step(&python, &RealFileSystem, &counted, &parameters, &work).unwrap();
    assert_eq!(read(&dir.join("calls")), "call\n");
    assert!(!RealFileSystem.exists(&dir.join("collects")).unwrap());
    execute_step(&python, &RealFileSystem, &counted, &inputs, &work).unwrap();
    assert_eq!(read(&dir.join("calls")), "call\n");
    assert_eq!(read(&dir.join("collects")), "collect\n");
}

#[test]
fn mg_req_0619_model_never_shadows_a_module() {
    let root = scratch("shadowing");
    write(&root.join("models").join("datetime.py"), DATETIME);
    write(&root.join("models").join("helpers.py"), HELPERS);
    write(
        &root.join("lib").join("helpers.py"),
        "import polars as pl\nusd = (pl.col('amount') * pl.col('rate')).alias('amount_usd')\n",
    );
    let work = root.join("work");
    let Some(python) = python(&root, &work) else {
        return;
    };
    let (_, inputs) = joined();
    let run = |python: &RealPython, file: &str| {
        let step = step_of(&root, file, file);
        execute_step(python, &RealFileSystem, &step, &inputs, &work).unwrap()
    };
    assert_eq!(
        column(&run(&python, "datetime").batches, "amount"),
        [10.0, 2.0]
    );
    assert_eq!(
        column(&run(&python, "helpers").batches, "amount_usd"),
        [20.0, 1.0]
    );
}

/// A step that leaves through `sys.exit`, and one that changes the current directory.
const RESTLESS: &str = r#"import os, sys

def exits(joined):
    sys.exit(3)

def wanders(joined):
    os.chdir(os.path.dirname(os.getcwd()))
    return joined
"#;

#[test]
fn mg_req_0617_exit_and_chdir_keep_the_report() {
    let root = scratch("restless");
    write(&root.join("models").join("restless.py"), RESTLESS);
    let work = root.join("work");
    let Some(python) = python(&root, &work) else {
        return;
    };
    let (parameters, inputs) = joined();
    let exits = step_of(&root, "restless", "exits");
    let error = plan_step(&python, &RealFileSystem, &exits, &parameters, &work).unwrap_err();
    assert!(
        matches!(&error, PolarsError::StepFailed { failure: StepFailure::Exception { kind, .. }, .. }
            if kind == "SystemExit"),
        "{error:?}"
    );
    let wanders = step_of(&root, "restless", "wanders");
    let output = execute_step(&python, &RealFileSystem, &wanders, &inputs, &work).unwrap();
    assert_eq!(column(&output.batches, "amount"), [10.0, 2.0]);
}
