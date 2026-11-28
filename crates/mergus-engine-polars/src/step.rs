//! The `subprocess` level: one Python child per call of a step (MG-REQ-0617.1).

use std::path::{Component, Path, PathBuf};

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use mergus_ports::{Environment, FileSystem, ProcessOutput, Python};
use serde::Deserialize;

use crate::{PolarsError, StepFailure, ipc};

/// The program the child runs. It imports only the standard library, `polars` and the model.
const CHILD: &str = include_str!("child.py");

/// The file the child writes the planned schema to, as a zero-row Arrow IPC file.
const SCHEMA_FILE: &str = "schema.arrow";

/// The file the child writes the collected result to.
const OUTPUT_FILE: &str = "output.arrow";

/// The file the child writes its report to.
const STATUS_FILE: &str = "status.json";

/// A Polars step: a function of a model's Python file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// The model, as errors name it.
    pub model: String,
    /// The step: the name of the function.
    pub step: String,
    /// The model's Python file, as an absolute path.
    pub module: PathBuf,
    /// The project's `lib` directory, as an absolute path; it comes first on `sys.path`.
    pub lib: PathBuf,
}

/// A parameter of a step and the schema of its input, for planning.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    /// The parameter's name; the input is passed as this keyword argument.
    pub name: String,
    /// The schema of the input.
    pub schema: SchemaRef,
}

/// A parameter of a step and the rows of its input, for execution.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The parameter's name; the input is passed as this keyword argument.
    pub name: String,
    /// The schema of the input.
    pub schema: SchemaRef,
    /// The rows of the input.
    pub batches: Vec<RecordBatch>,
}

/// The collected result of a step.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    /// Its schema, as Polars wrote it.
    pub schema: SchemaRef,
    /// Its rows.
    pub batches: Vec<RecordBatch>,
}

/// What the child reports in its status file.
#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Status {
    Ok,
    NoPolars {
        #[serde(rename = "type")]
        kind: String,
        message: String,
    },
    NotLazyFrame {
        returned: String,
    },
    LoadFailed {
        #[serde(rename = "type")]
        kind: String,
        message: String,
    },
    Exception {
        #[serde(rename = "type")]
        kind: String,
        message: String,
    },
}

/// Returns the working directory of `step` in the run whose work directory is `work_dir`:
/// `<work_dir>/polars/<model>/<step>`.
///
/// # Errors
///
/// Fails with [`PolarsError::InvalidName`] when the model or the step is not exactly one
/// plain path component: empty, `.`, `..`, absolute, with a separator or a Windows prefix.
pub fn step_dir(work_dir: &Path, step: &Step) -> Result<PathBuf, PolarsError> {
    let dir = work_dir
        .join("polars")
        .join(component(&step.model)?)
        .join(component(&step.step)?);
    Ok(dir)
}

/// Returns `name` when it is exactly one plain path component.
fn component(name: &str) -> Result<&str, PolarsError> {
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(part)), None) if part == name => Ok(name),
        _ => Err(PolarsError::InvalidName {
            name: name.to_owned(),
        }),
    }
}

/// Returns the whole environment of the Python child of a run whose work directory is
/// `work_dir`: the allowlisted one of `mergus_security::python_command`, read from
/// `environment`, with no secret value in it (MG-REQ-0006, MG-REQ-0617.1). The caller gives
/// it to the interpreter it locates.
#[must_use]
pub fn child_environment(work_dir: &str, environment: &dyn Environment) -> Vec<(String, String)> {
    mergus_security::python_command("python3", &[], work_dir, environment).env
}

/// Calls `step` once with an empty LazyFrame of each parameter's schema, as keyword
/// arguments, and returns the schema of the LazyFrame it returns (MG-REQ-0617.2). The call
/// runs in a fresh [`step_dir`] under `work_dir`.
///
/// # Errors
///
/// Fails with [`PolarsError::StepFailed`] (`MG-0627`) when the step raises an exception or
/// does not return a LazyFrame, with [`PolarsError::LoadFailed`] (`MG-0628`) when the model
/// file raises when imported or has no function named after the step, with
/// [`PolarsError::PolarsMissing`] (`MG-0629`) when the
/// child cannot import `polars`, with [`PolarsError::InvalidName`] when the model or the step
/// cannot name a directory, with [`PolarsError::Child`] when the child stops without a
/// report, with [`PolarsError::Boundary`] when a file of the step cannot be read as Arrow
/// IPC, and with [`PolarsError::Port`] when the interpreter or the file system fails.
pub fn plan_step(
    python: &dyn Python,
    files: &dyn FileSystem,
    step: &Step,
    parameters: &[Parameter],
    work_dir: &Path,
) -> Result<SchemaRef, PolarsError> {
    let dir = fresh_dir(files, work_dir, step)?;
    let mut args = Vec::new();
    for (index, parameter) in parameters.iter().enumerate() {
        let path = dir.join(input_file(index));
        ipc::write(files, &path, &parameter.schema, &[])?;
        args.push((parameter.name.clone(), path));
    }
    call(python, files, step, "plan", &args, &dir)?;
    let (schema, _) = ipc::read(files, &dir.join(SCHEMA_FILE))?;
    Ok(schema)
}

/// Writes each input as an Arrow IPC file, calls `step` once with `pl.scan_ipc(<file>)` for
/// each, as keyword arguments, and returns the result the child collected once
/// (MG-REQ-0617.3). The call runs in a fresh [`step_dir`] under `work_dir`.
///
/// # Errors
///
/// Fails as [`plan_step`] does.
pub fn execute_step(
    python: &dyn Python,
    files: &dyn FileSystem,
    step: &Step,
    inputs: &[Frame],
    work_dir: &Path,
) -> Result<Output, PolarsError> {
    let dir = fresh_dir(files, work_dir, step)?;
    let mut args = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        let path = dir.join(input_file(index));
        ipc::write(files, &path, &input.schema, &input.batches)?;
        args.push((input.name.clone(), path));
    }
    call(python, files, step, "execute", &args, &dir)?;
    let (schema, batches) = ipc::read(files, &dir.join(OUTPUT_FILE))?;
    Ok(Output { schema, batches })
}

/// The file of the input at `index`.
fn input_file(index: usize) -> String {
    format!("input-{index}.arrow")
}

/// Removes what an earlier call left in the step's directory and creates it empty
/// (MG-REQ-0636.1). It never removes a directory outside `work_dir`.
fn fresh_dir(files: &dyn FileSystem, work_dir: &Path, step: &Step) -> Result<PathBuf, PolarsError> {
    let dir = step_dir(work_dir, step)?;
    if !dir.starts_with(work_dir) {
        return Err(PolarsError::InvalidName {
            name: dir.display().to_string(),
        });
    }
    if files.exists(&dir)? {
        files.remove_dir_all(&dir)?;
    }
    files.create_dir_all(&dir)?;
    Ok(dir)
}

/// Runs the child in `dir` and turns its report into a result.
fn call(
    python: &dyn Python,
    files: &dyn FileSystem,
    step: &Step,
    mode: &str,
    inputs: &[(String, PathBuf)],
    dir: &Path,
) -> Result<(), PolarsError> {
    let mut args = vec![
        mode.to_owned(),
        step.module.display().to_string(),
        step.lib.display().to_string(),
        step.step.clone(),
    ];
    for (name, path) in inputs {
        args.push(name.clone());
        args.push(path.display().to_string());
    }
    let output = python.run(CHILD, &args, dir)?;
    let status = dir.join(STATUS_FILE);
    let report = if files.exists(&status)? {
        serde_json::from_slice::<Status>(&files.read(&status)?).ok()
    } else {
        None
    };
    outcome(step, report, &output)
}

/// Turns the child's report, or its absence, into the result of the call.
fn outcome(step: &Step, report: Option<Status>, output: &ProcessOutput) -> Result<(), PolarsError> {
    let failure = |failure| PolarsError::StepFailed {
        model: step.model.clone(),
        step: step.step.clone(),
        failure,
    };
    match report {
        Some(Status::Ok) if output.code == Some(0) => Ok(()),
        Some(Status::NoPolars { kind, message }) => Err(PolarsError::PolarsMissing {
            model: step.model.clone(),
            step: step.step.clone(),
            kind,
            message,
        }),
        Some(Status::LoadFailed { kind, message }) => Err(PolarsError::LoadFailed {
            model: step.model.clone(),
            file: step.module.display().to_string(),
            kind,
            message,
        }),
        Some(Status::NotLazyFrame { returned }) => {
            Err(failure(StepFailure::NotLazyFrame { returned }))
        }
        Some(Status::Exception { kind, message }) => {
            Err(failure(StepFailure::Exception { kind, message }))
        }
        Some(Status::Ok) | None => Err(PolarsError::Child {
            model: step.model.clone(),
            step: step.step.clone(),
            reason: stopped(output),
        }),
    }
}

/// Describes a child that stopped without a report.
fn stopped(output: &ProcessOutput) -> String {
    let code = output
        .code
        .map_or_else(|| "a signal".to_owned(), |code| format!("exit code {code}"));
    format!(
        "it stopped with {code}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
}
