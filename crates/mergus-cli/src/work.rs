//! The read project mapped onto the runtime's [`Work`]: one [`SqlWork`] per `.sql` model the
//! plan computes, through `mergus-engine-duckdb`, with each output schema as DuckDB
//! describes it against empty inputs (MG-REQ-0604).

use std::collections::{BTreeMap, BTreeSet};

use mergus_contracts::generated::mergus_plan_v1::StepAction;
use mergus_plan::Plan;
use mergus_ports::{Database, ResultColumn};
use mergus_project::Seed;
use mergus_project::{KnownTables, LayoutError, Macros, RunParameters, read_sql_step, sql_schema};
use mergus_runtime::{ModelWork, ReadModel, ReadProject, SqlInput, SqlWork, Work};
use mergus_source::copied_tables;

use crate::arrow_types::schema;
use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::run::Ports;

/// The code of a column type a segment cannot hold exactly (MG-REQ-0604).
const UNREPRESENTABLE: &str = "MG-0101";

/// What the work of a run needs besides the project.
pub(crate) struct Context<'a> {
    pub(crate) ports: &'a Ports<'a>,
    pub(crate) database: &'a dyn Database,
    /// The run parameters every step binds (MG-REQ-0603).
    pub(crate) parameters: RunParameters,
}

/// The work of every step `plan` computes. Returns the diagnostics of a model whose schema
/// cannot be described or carried.
///
/// # Errors
///
/// Fails with [`CliError::Internal`], naming what is missing, for a plan the run cannot
/// execute yet: a model that reads `raw.<table>` (#316), a Polars step (#317) or an
/// extraction (#318); and when a port fails.
pub(crate) fn work<'a>(
    context: &Context<'a>,
    project: &ReadProject,
    plan: &Plan,
) -> Result<Result<Work<'a>, Vec<Diagnostic>>, CliError> {
    let known = KnownTables::new(&project.layout, &project.configuration);
    let macros = macro_texts(context.ports, project)?;
    let mut columns: BTreeMap<String, Vec<ResultColumn>> = BTreeMap::new();
    let mut models: BTreeMap<String, Box<dyn ModelWork + 'a>> = BTreeMap::new();
    let extracted = extracted(project);
    for step in &plan.document().steps {
        let model = project
            .models
            .iter()
            .find(|model| model.options.name == step.model)
            .ok_or_else(|| CliError::Internal(format!("no model `{}`", step.model)))?;
        if let Some(load) = step.inputs.iter().find(|input| extracted.contains(*input)) {
            return Err(CliError::Internal(format!(
                "`{}` reads `{load}`, whose extraction a run does not do yet (#318)",
                model.options.path
            )));
        }
        let sql = match described(context, &known, model, &columns)? {
            Ok(described) => described,
            Err(diagnostics) => return Ok(Err(diagnostics)),
        };
        if step.action == StepAction::Compute {
            let work = match sql_work(context, model, &sql, &columns, macros.clone()) {
                Ok(work) => work,
                Err(diagnostic) => return Ok(Err(vec![*diagnostic])),
            };
            models.insert(step.model.clone(), Box::new(work));
        }
        columns.insert(step.model.clone(), sql.columns);
    }
    Ok(Ok(Work {
        extractions: Vec::new(),
        raw_snapshot: None,
        models,
        // #307 measures the DuckLake files the plan reads; until then nothing is counted.
        bytes_scanned: 0,
    }))
}

/// The tables an extraction loads before any segment reads them: each copied table of a
/// source, as `raw.<source>__<table>`, and each seed, as `raw.seed__<name>` (MG-REQ-0704).
fn extracted(project: &ReadProject) -> BTreeSet<String> {
    let tables = project.configuration.sources.iter().flat_map(|source| {
        copied_tables(source)
            .into_iter()
            .map(|table| table.raw(&source.name).current)
    });
    tables
        .chain(project.seeds.seeds.iter().map(Seed::table))
        .collect()
}

/// A `.sql` model read as a step, with the columns of its result.
struct Described {
    sql: String,
    params: Vec<mergus_ports::SqlValue>,
    columns: Vec<ResultColumn>,
}

/// Reads `model` as a step and describes its result against the columns of its inputs,
/// which are steps already described.
fn described(
    context: &Context<'_>,
    known: &KnownTables,
    model: &ReadModel,
    columns: &BTreeMap<String, Vec<ResultColumn>>,
) -> Result<Result<Described, Vec<Diagnostic>>, CliError> {
    let path = &model.options.path;
    if !path.ends_with(".sql") {
        return Err(CliError::Internal(format!(
            "`{path}` is a Polars step, which a run does not execute yet (#317)"
        )));
    }
    let mut inputs = BTreeMap::new();
    for input in &model.inputs {
        let found = columns.get(input).ok_or_else(|| {
            CliError::Internal(format!(
                "`{path}` reads the raw input `{input}`, which a SQL segment does not read yet (#316)"
            ))
        })?;
        inputs.insert(input.clone(), found.clone());
    }
    let parser = context.ports.sql_parser;
    let step = match read_sql_step(path, model.code.as_bytes(), known, parser) {
        Ok(step) => step,
        Err(error) => return layout(error),
    };
    let described = match sql_schema(context.database, &step, &inputs) {
        Ok(described) => described,
        Err(error) => return layout(error),
    };
    Ok(Ok(Described {
        params: step.bind(&context.parameters),
        sql: step.sql,
        columns: described,
    }))
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

/// The DuckDB work of `model`, whose inputs are the described steps of `columns`.
fn sql_work<'a>(
    context: &Context<'a>,
    model: &ReadModel,
    sql: &Described,
    columns: &BTreeMap<String, Vec<ResultColumn>>,
    macros: Vec<String>,
) -> Result<SqlWork<'a>, Box<Diagnostic>> {
    let carried = |name: &str, found: &[ResultColumn]| {
        schema(found).map_err(|(column, data_type)| {
            Box::new(Diagnostic::of_code(
                UNREPRESENTABLE,
                format!("model `{name}`: column `{column}` has type `{data_type}`, which a run cannot carry"),
            ))
        })
    };
    let mut inputs = Vec::with_capacity(model.inputs.len());
    for input in &model.inputs {
        let found = columns.get(input).map(Vec::as_slice).unwrap_or_default();
        inputs.push(SqlInput {
            name: input.clone(),
            schema: carried(input, found)?,
            boundary: None,
        });
    }
    Ok(SqlWork {
        database: context.database,
        files: context.ports.files,
        sql: sql.sql.clone(),
        params: sql.params.clone(),
        macros,
        catalogs: Vec::new(),
        inputs,
        output: carried(&model.options.name, &sql.columns)?,
    })
}

/// The text of every macro file of the project, each after the macros it calls.
fn macro_texts(ports: &Ports<'_>, project: &ReadProject) -> Result<Vec<String>, CliError> {
    let mut texts = Vec::new();
    let mut done = BTreeSet::new();
    for name in project.macros.macros.keys() {
        load(ports, project, &project.macros, name, &mut done, &mut texts)?;
    }
    Ok(texts)
}

/// Appends the text of macro `name` to `texts`, after the macros it calls.
fn load(
    ports: &Ports<'_>,
    project: &ReadProject,
    macros: &Macros,
    name: &str,
    done: &mut BTreeSet<String>,
    texts: &mut Vec<String>,
) -> Result<(), CliError> {
    let Some(found) = macros.macros.get(name) else {
        return Ok(());
    };
    if !done.insert(name.to_owned()) {
        return Ok(());
    }
    for called in &found.calls {
        load(ports, project, macros, called, done, texts)?;
    }
    let bytes = ports
        .files
        .read(&project.root.join(&found.path))
        .map_err(|error| CliError::Unreadable(error.to_string()))?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if !texts.contains(&text) {
        texts.push(text);
    }
    Ok(())
}
