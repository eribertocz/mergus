//! `.py` models read through the tree of the [`Python`] port, never imported (MG-REQ-0611):
//! public functions are steps (MG-REQ-0612), the return decides the engine (MG-REQ-0613),
//! parameter names wire the steps (MG-REQ-0614), the function named like the file is the
//! result (MG-REQ-0615), steps never make data eager (MG-REQ-0616), and the `lib` modules a
//! model imports are fingerprint inputs (MG-REQ-0618).

use std::collections::{BTreeMap, BTreeSet};

use mergus_ports::{PortError, Python, PythonFunction, PythonTree};

use crate::{KnownTables, LayoutError, ProjectError, PythonModelError};

mod eager;
mod lib_modules;
mod returns;
mod wiring;

pub use lib_modules::{PythonLib, PythonLibModule, read_python_lib};

/// A `.py` model whose steps passed the checks of MG-REQ-0612 to MG-REQ-0616.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonModel {
    /// The path of the model, relative to the project root.
    pub path: String,
    /// The model name, the file stem: the name of the step that is its result (MG-REQ-0615).
    pub name: String,
    /// Its steps, in the order of the file (MG-REQ-0612.1).
    pub steps: Vec<PythonStep>,
    /// Its fingerprint inputs from `lib`: `lib:<module>` to `sha256:<hex>` of each module it
    /// imports, directly or through other `lib` modules (MG-REQ-0618.2).
    pub lib_inputs: BTreeMap<String, String>,
}

impl PythonModel {
    /// Returns the step that is the model's result (MG-REQ-0615).
    #[must_use]
    pub fn result(&self) -> Option<&PythonStep> {
        self.steps.iter().find(|step| step.name == self.name)
    }
}

/// One step: a public top-level function (MG-REQ-0612).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonStep {
    /// Its name.
    pub name: String,
    /// The line of its `def`.
    pub line: usize,
    /// The engine its returns select (MG-REQ-0613).
    pub engine: StepEngine,
    /// What each parameter reads, in the order of the parameters (MG-REQ-0614).
    pub inputs: Vec<StepInput>,
}

/// The engine of a step (MG-REQ-0613).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepEngine {
    /// Every return is a string literal: DuckDB runs it.
    DuckDb {
        /// The text of each literal returned, in the order of the returns.
        sql: Vec<String>,
    },
    /// Every return is another expression, a `LazyFrame` confirmed when the step runs.
    Polars,
}

/// What a parameter of a step reads (MG-REQ-0614).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepInput {
    /// The parameter name.
    pub parameter: String,
    /// What the name resolves to.
    pub source: InputSource,
}

/// What a parameter name resolves to, in this order of precedence (MG-REQ-0614).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputSource {
    /// Another step of the same file.
    Step(String),
    /// A model of the project.
    Model(String),
    /// A raw table of a source or a seed, named without its schema: `erp__sales`,
    /// `seed__countries`.
    Raw(String),
}

/// Reads the `.py` model at `path` holding `bytes` through `python`, which parses it without
/// importing it (MG-REQ-0611). Its parameters resolve against its own steps and `known`;
/// `lib` gives the fingerprints of the `lib` modules it imports.
///
/// # Errors
///
/// Fails with [`LayoutError::Invalid`] carrying the syntax error of a file that is not valid
/// Python, or else every [`PythonModelError`] of the file: async steps, invalid returns,
/// parameters, cycles, the missing result, then eager calls, each group in the order of the
/// file. An async step is reported once: neither its readers nor a missing result repeat it.
/// Fails with [`LayoutError::Port`] when `python` fails otherwise.
pub fn read_python_model(
    path: &str,
    bytes: &[u8],
    known: &KnownTables,
    lib: &PythonLib,
    python: &dyn Python,
) -> Result<PythonModel, LayoutError> {
    let tree = parse(path, bytes, python)?;
    let name = stem(path);
    let mut problems = Vec::new();
    let functions = steps_of(path, &tree, &mut problems);
    let rejected = rejected(&tree);
    let mut steps = Vec::new();
    for function in &functions {
        let engine = returns::engine(path, function, &tree, &mut problems);
        steps.push(PythonStep {
            name: function.name.clone(),
            line: to_usize(function.line),
            engine: engine.unwrap_or(StepEngine::Polars),
            inputs: Vec::new(),
        });
    }
    let names = wiring::Names {
        model: name,
        rejected: &rejected,
        known,
    };
    wiring::wire(path, &names, &functions, &mut steps, &mut problems);
    if !steps.iter().any(|step| step.name == name) && !rejected.contains(name) {
        problems.push(PythonModelError::MissingResult {
            path: path.to_owned(),
            stem: name.to_owned(),
        });
    }
    eager::check(path, &tree, &mut problems);
    if !problems.is_empty() {
        return Err(LayoutError::Invalid(
            problems.into_iter().map(ProjectError::Python).collect(),
        ));
    }
    Ok(PythonModel {
        path: path.to_owned(),
        name: name.to_owned(),
        steps,
        lib_inputs: lib.fingerprint_inputs(&tree.imports),
    })
}

/// The tree of the file at `path` holding `bytes`. A syntax error is a problem of the file
/// (MG-REQ-0611.1).
pub(super) fn parse(
    path: &str,
    bytes: &[u8],
    python: &dyn Python,
) -> Result<PythonTree, LayoutError> {
    match python.parse(&String::from_utf8_lossy(bytes)) {
        Ok(tree) => Ok(tree),
        Err(PortError::PythonSyntax {
            message,
            line,
            column,
        }) => Err(LayoutError::Invalid(vec![ProjectError::Python(
            PythonModelError::PythonSyntax {
                path: path.to_owned(),
                line: line.map(to_usize),
                column: column.map(to_usize),
                message,
            },
        )])),
        Err(error) => Err(error.into()),
    }
}

/// The steps of `tree`: its public top-level functions, in order. A public `async def` is a
/// problem and no step (MG-REQ-0612.1).
fn steps_of<'a>(
    path: &str,
    tree: &'a PythonTree,
    problems: &mut Vec<PythonModelError>,
) -> Vec<&'a PythonFunction> {
    let mut steps = Vec::new();
    for function in tree.functions.iter().filter(|f| !f.name.starts_with('_')) {
        if function.is_async {
            problems.push(PythonModelError::AsyncStep {
                path: path.to_owned(),
                step: function.name.clone(),
                line: to_usize(function.line),
            });
        } else {
            steps.push(function);
        }
    }
    steps
}

/// The public `async def` functions of `tree`: rejected steps (MG-REQ-0612.1).
fn rejected(tree: &PythonTree) -> BTreeSet<&str> {
    tree.functions
        .iter()
        .filter(|function| function.is_async && !function.name.starts_with('_'))
        .map(|function| function.name.as_str())
        .collect()
}

/// The stem of the file at `path`: its name without its last extension.
fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// A line or column of the tree as a `usize`.
fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}
