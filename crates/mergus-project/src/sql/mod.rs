//! The checks of `.sql` models on DuckDB's tree (MG-REQ-0193): every table they read is
//! known to the project (MG-REQ-0307, MG-REQ-0373), and they are one read-only `SELECT`
//! (MG-REQ-0308, MG-REQ-0374).

mod cycle;
mod known;
mod step;
mod tree;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mergus_ports::{FileSystem, SqlParser};
use serde_json::Value;

pub use known::KnownTables;
pub use step::{RunParameters, SqlStep, read_sql_step, sql_schema};

use crate::{
    Configuration, InputSource, Layout, LayoutError, ModelLanguage, ProjectError, PythonModel,
};

/// Checks every `.sql` model of `layout`, parsed by `parser`, against the tables that
/// `configuration` declares.
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when a model cannot be read or the parser cannot be
/// reached, and with [`LayoutError::Invalid`] carrying every problem found: the models in the
/// order of `layout`, the problems of each model ordered by line and column.
pub fn check_sql_models(
    files: &dyn FileSystem,
    root: &Path,
    layout: &Layout,
    configuration: &Configuration,
    parser: &dyn SqlParser,
) -> Result<(), LayoutError> {
    let known = KnownTables::new(layout, configuration);
    let mut problems = Vec::new();
    for model in &layout.models {
        if model.language == ModelLanguage::Sql {
            let bytes = files.read(&root.join(&model.path))?;
            problems.extend(check_sql(&model.path, &bytes, &known, parser)?);
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(LayoutError::Invalid(problems))
    }
}

/// Returns the problems of the `.sql` model at `path` holding `bytes`, ordered by line, then
/// column, then code.
///
/// # Errors
///
/// Fails when the parser cannot be reached.
pub fn check_sql(
    path: &str,
    bytes: &[u8],
    known: &KnownTables,
    parser: &dyn SqlParser,
) -> Result<Vec<ProjectError>, LayoutError> {
    walk_sql(path, bytes, known, parser).map(|checked| checked.problems)
}

/// Returns the resolved references of every `.sql` model of `layout`, parsed by `parser`,
/// by model name (MG-REQ-0373): lowercase, one per table read, a model bare and a raw table
/// as `raw.<table>`, sorted. CTEs are not references. Python models are not listed.
///
/// # Errors
///
/// As [`read_model_references`] without `.py` models.
pub fn read_sql_references(
    files: &dyn FileSystem,
    root: &Path,
    layout: &Layout,
    configuration: &Configuration,
    parser: &dyn SqlParser,
) -> Result<BTreeMap<String, Vec<String>>, LayoutError> {
    read_model_references(files, root, layout, configuration, parser, &[])
}

/// Returns the references of every `.sql` model of `layout`, as [`read_sql_references`]
/// does, and of every `.py` model of `python`, already read: the models and raw tables its
/// parameters read, named the same way. Cycles are found over both in one pass
/// (MG-REQ-0375.1).
///
/// # Errors
///
/// Fails as [`check_sql_models`] does, with every problem of every `.sql` model when one has
/// any, followed by one [`ProjectError::ModelCycle`] for each group of models, `.sql` or
/// `.py`, that read one another in a cycle (MG-REQ-0375).
pub fn read_model_references(
    files: &dyn FileSystem,
    root: &Path,
    layout: &Layout,
    configuration: &Configuration,
    parser: &dyn SqlParser,
    python: &[PythonModel],
) -> Result<BTreeMap<String, Vec<String>>, LayoutError> {
    let known = KnownTables::new(layout, configuration);
    let mut problems = Vec::new();
    let mut found = Vec::new();
    for model in &layout.models {
        if model.language == ModelLanguage::Sql {
            let bytes = files.read(&root.join(&model.path))?;
            let checked = walk_sql(&model.path, &bytes, &known, parser)?;
            problems.extend(checked.problems);
            found.push((model, checked.references));
        }
    }
    problems.extend(cycle::cycles(&found, python));
    if !problems.is_empty() {
        return Err(LayoutError::Invalid(problems));
    }
    let sql = found
        .into_iter()
        .map(|(model, references)| (model.name.clone(), references.into_keys().collect()));
    let python = python
        .iter()
        .map(|model| (model.name.clone(), python_references(model)));
    Ok(sql.chain(python).collect())
}

/// The tables the parameters of `model` read: each model bare, each raw table or seed as
/// `raw.<table>`, sorted, each once. Its own steps are not references.
fn python_references(model: &PythonModel) -> Vec<String> {
    let names = model.steps.iter().flat_map(|step| &step.inputs);
    let names = names.filter_map(|input| match &input.source {
        InputSource::Step(_) => None,
        InputSource::Model(name) => Some(name.clone()),
        InputSource::Raw(table) => Some(format!("raw.{table}")),
    });
    names.collect::<BTreeSet<_>>().into_iter().collect()
}

/// Walks the `.sql` model at `path` holding `bytes`: its problems ordered by line, then
/// column, then code, and its resolved references.
fn walk_sql(
    path: &str,
    bytes: &[u8],
    known: &KnownTables,
    parser: &dyn SqlParser,
) -> Result<tree::Checked, LayoutError> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        let at = Position { line: 1, column: 1 };
        return Ok(tree::Checked {
            problems: vec![forbidden(path, at, "text that is not UTF-8".to_owned())],
            references: std::collections::BTreeMap::new(),
            parameters: Vec::new(),
            instants: Vec::new(),
        });
    };
    let tree = parser.parse(text)?;
    let mut checked = match serde_json::from_str::<Value>(&tree) {
        Ok(tree) => tree::check(path, text, &tree, known),
        Err(error) => {
            return Err(LayoutError::Port(mergus_ports::PortError::Failed(format!(
                "the SQL parser returned a tree that is not JSON: {error}"
            ))));
        }
    };
    checked.problems.sort_by(|left, right| {
        (left.line(), left.column(), left.code()).cmp(&(right.line(), right.column(), right.code()))
    });
    Ok(checked)
}

/// A line and a column of a text, both from 1, the column counted in Unicode scalar values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Position {
    pub(crate) line: usize,
    pub(crate) column: usize,
}

impl Position {
    /// The position of the byte `offset` of `text`; the start of the text when `offset` is
    /// not in it.
    pub(crate) fn of(text: &str, offset: usize) -> Self {
        let Some(before) = text.get(..offset) else {
            return Self { line: 1, column: 1 };
        };
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        Self {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

/// The error of MG-REQ-0374 for `construct`, at `at` of `path`.
pub(crate) fn forbidden(path: &str, at: Position, construct: String) -> ProjectError {
    ProjectError::ForbiddenSql {
        path: path.to_owned(),
        line: at.line,
        column: at.column,
        construct,
    }
}
