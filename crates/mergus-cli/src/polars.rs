//! A `.py` model of the read project mapped onto the runtime's [`PolarsWork`]: the Polars
//! steps its result reads, run in a Python child each, with each output schema as the step
//! plans it against empty inputs (MG-REQ-0617.2).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use arrow::datatypes::SchemaRef;
use mergus_ports::{Python, ResultColumn};
use mergus_project::{
    InputSource, KnownTables, LayoutError, PythonModel, PythonStep, StepEngine, read_python_lib,
    read_python_model,
};
use mergus_runtime::{PolarsInput, PolarsSource, PolarsStep, PolarsWork, ReadModel, ReadProject};

use crate::arrow_types::schema;
use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::work::{Context, UNREPRESENTABLE};

/// The folder of the project's own Python modules.
const LIB: &str = "lib";

/// The Polars work of the `.py` `model` and the schema of its result, planned in a fresh
/// directory under the run's scratch directory. `columns` are the described `.sql` steps and
/// `schemas` the planned `.py` ones, which its inputs are.
///
/// # Errors
///
/// Fails with [`CliError::Internal`], naming what is missing, for a model the run cannot
/// execute yet: one that reads `raw.<table>` (#316) or has a step that returns SQL; and when
/// a port fails.
pub(crate) fn polars_work<'a>(
    context: &Context<'a>,
    project: &ReadProject,
    model: &ReadModel,
    columns: &BTreeMap<String, Vec<ResultColumn>>,
    schemas: &BTreeMap<String, SchemaRef>,
) -> Result<Result<(PolarsWork<'a>, SchemaRef), Vec<Diagnostic>>, CliError> {
    let path = &model.options.path;
    let python = context.python.ok_or_else(|| {
        CliError::Internal(format!("`{path}` is a Polars step, but no Python was located"))
    })?;
    let read = match python_model(context, project, python, model) {
        Ok(read) => read,
        Err(error) => return layout(error),
    };
    let steps = steps(&read)?;
    let mut inputs = BTreeMap::new();
    for step in &steps {
        for input in &step.inputs {
            if let PolarsSource::Model(name) = &input.source {
                match input_schema(name, columns, schemas) {
                    Ok(found) => inputs.insert(name.clone(), found),
                    Err(diagnostic) => return Ok(Err(vec![*diagnostic])),
                };
            }
        }
    }
    let work = PolarsWork {
        python,
        files: context.ports.files,
        model: model.options.name.clone(),
        module: under(&project.root, path),
        lib: project.root.join(LIB),
        steps,
        schemas: inputs,
    };
    match work.describe(&context.scratch.join("plan")) {
        Ok(schema) => Ok(Ok((work, schema))),
        Err(failure) => match failure.code {
            Some(code) => Ok(Err(vec![Diagnostic::of_code(code, failure.message)])),
            None => Err(CliError::Internal(failure.to_string())),
        },
    }
}

/// Reads `model` again as a `.py` model, with the project's `lib`.
fn python_model(
    context: &Context<'_>,
    project: &ReadProject,
    python: &dyn Python,
    model: &ReadModel,
) -> Result<PythonModel, LayoutError> {
    let files = context.ports.files;
    let lib = read_python_lib(files, &project.root, python)?;
    let known = KnownTables::new(&project.layout, &project.configuration);
    let path = &model.options.path;
    read_python_model(path, model.code.as_bytes(), &known, &lib, python)
}

/// The steps the result of `model` reads, directly or through other steps, each after the
/// steps it reads, in the order of their parameters; the result is the last.
fn steps(model: &PythonModel) -> Result<Vec<PolarsStep>, CliError> {
    let result = model.result().ok_or_else(|| {
        CliError::Internal(format!("`{}` has no step `{}`", model.path, model.name))
    })?;
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    visit(model, result, &mut seen, &mut ordered)?;
    Ok(ordered)
}

/// Appends `step` to `ordered` after the steps it reads.
fn visit(
    model: &PythonModel,
    step: &PythonStep,
    seen: &mut BTreeSet<String>,
    ordered: &mut Vec<PolarsStep>,
) -> Result<(), CliError> {
    if !seen.insert(step.name.clone()) {
        return Ok(());
    }
    if let StepEngine::DuckDb { .. } = step.engine {
        return Err(CliError::Internal(format!(
            "`{}`: step `{}` returns SQL, which a run does not execute in a `.py` model yet",
            model.path, step.name
        )));
    }
    let mut inputs = Vec::with_capacity(step.inputs.len());
    for input in &step.inputs {
        let source = match &input.source {
            InputSource::Step(name) => {
                let read = model.steps.iter().find(|found| found.name == *name);
                let read = read.ok_or_else(|| {
                    CliError::Internal(format!("`{}` has no step `{name}`", model.path))
                })?;
                visit(model, read, seen, ordered)?;
                PolarsSource::Step(name.clone())
            }
            InputSource::Model(name) => PolarsSource::Model(name.clone()),
            InputSource::Raw(table) => {
                return Err(CliError::Internal(format!(
                    "`{}` reads the raw input `raw.{table}`, which a Polars step does not read yet (#316)",
                    model.path
                )));
            }
        };
        inputs.push(PolarsInput {
            parameter: input.parameter.clone(),
            source,
        });
    }
    ordered.push(PolarsStep {
        name: step.name.clone(),
        inputs,
    });
    Ok(())
}

/// The schema of the model `name`, a `.py` step planned or a `.sql` step described.
fn input_schema(
    name: &str,
    columns: &BTreeMap<String, Vec<ResultColumn>>,
    schemas: &BTreeMap<String, SchemaRef>,
) -> Result<SchemaRef, Box<Diagnostic>> {
    if let Some(found) = schemas.get(name) {
        return Ok(found.clone());
    }
    let found = columns.get(name).map(Vec::as_slice).unwrap_or_default();
    schema(found).map_err(|(column, data_type)| {
        Box::new(Diagnostic::of_code(
            UNREPRESENTABLE,
            format!(
                "model `{name}`: column `{column}` has type `{data_type}`, which a run cannot carry"
            ),
        ))
    })
}

/// `relative`, a path separated by `/`, under `root`.
fn under(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

/// The diagnostics of `error`, or the failure of its port.
fn layout<T>(error: LayoutError) -> Result<Result<T, Vec<Diagnostic>>, CliError> {
    match error {
        LayoutError::Invalid(errors) => {
            Ok(Err(errors.iter().map(Diagnostic::of_project).collect()))
        }
        other => Err(CliError::Unreadable(other.to_string())),
    }
}
