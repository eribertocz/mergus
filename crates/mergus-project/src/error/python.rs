//! The problems of a `.py` model (MG-REQ-0612 to MG-REQ-0616).

use super::format::{cycle_names, hint};

/// A problem of a `.py` model. Paths are relative to the project root, separated by `/`;
/// lines and columns start at 1, a column counting characters.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PythonModelError {
    /// A public top-level function is an `async def` (MG-REQ-0612.1).
    #[error(
        "`{path}` line {line}: step `{step}` is an `async def`; a step is a plain `def`, or \
         rename it `_{step}` to make it a private helper"
    )]
    AsyncStep {
        /// The path of the model.
        path: String,
        /// The name of the function.
        step: String,
        /// The line of its `def`.
        line: usize,
    },
    /// A step returns neither a string literal nor a Polars expression, has no `return`, or
    /// mixes both kinds (MG-REQ-0613.2).
    #[error("`{path}` line {line}: step `{step}` {problem}")]
    InvalidReturn {
        /// The path of the model.
        path: String,
        /// The name of the step.
        step: String,
        /// The line of the `return`, or of the `def` of a step with no `return`.
        line: usize,
        /// What the step returns, and what it should return instead.
        problem: String,
    },
    /// A parameter names no step of the file, model, raw table or seed (MG-REQ-0614.1).
    #[error(
        "`{path}` line {line} column {column}: parameter `{parameter}` of step `{step}` is not \
         a step of this file, a model, a raw table or a seed{}",
        hint(suggestion.as_deref())
    )]
    UnresolvedParameter {
        /// The path of the model.
        path: String,
        /// The name of the step.
        step: String,
        /// The name of the parameter.
        parameter: String,
        /// The line of the parameter.
        line: usize,
        /// The column of the parameter.
        column: usize,
        /// The closest known name, within 3 edits (MG-REQ-0614.1).
        suggestion: Option<String>,
    },
    /// A parameter is `*args` or `**kwargs` (MG-REQ-0614.2).
    #[error(
        "`{path}` line {line} column {column}: parameter `{parameter}` of step `{step}` takes \
         any number of inputs; name each input as its own parameter"
    )]
    VariadicParameter {
        /// The path of the model.
        path: String,
        /// The name of the step.
        step: String,
        /// The parameter as written, with its `*` or `**`.
        parameter: String,
        /// The line of the parameter.
        line: usize,
        /// The column of the parameter.
        column: usize,
    },
    /// A parameter is positional-only, before `/`: inputs are passed by keyword
    /// (MG-REQ-0614.2).
    #[error(
        "`{path}` line {line} column {column}: parameter `{parameter}` of step `{step}` is \
         positional-only; Mergus passes every input by keyword, so move it after the `/`"
    )]
    PositionalOnlyParameter {
        /// The path of the model.
        path: String,
        /// The name of the step.
        step: String,
        /// The name of the parameter.
        parameter: String,
        /// The line of the parameter.
        line: usize,
        /// The column of the parameter.
        column: usize,
    },
    /// The steps of one file read each other in a cycle (MG-REQ-0614.2).
    #[error("`{path}` line {line}: steps {} form a cycle", cycle_names(steps))]
    StepCycle {
        /// The path of the model.
        path: String,
        /// The steps of the cycle, each reading the next, from the first in the file.
        steps: Vec<String>,
        /// The line of the `def` of the first step.
        line: usize,
    },
    /// No step is named like the file (MG-REQ-0615).
    #[error("`{path}` has no function `{stem}`; define `def {stem}(...)` for the model's result")]
    MissingResult {
        /// The path of the model.
        path: String,
        /// The file stem, the name the function must have.
        stem: String,
    },
    /// A model or `lib` file is not valid Python (MG-REQ-0611.1).
    #[error("`{path}`{}: not valid Python: {message}", location(*line, *column))]
    PythonSyntax {
        /// The path of the file.
        path: String,
        /// The line of the error, when Python gives one.
        line: Option<usize>,
        /// The column of the error, when Python gives one.
        column: Option<usize>,
        /// Python's message.
        message: String,
    },
    /// A model file makes data eager (MG-REQ-0616).
    #[error(
        "`{path}` line {line} column {column}: `{call}` makes the data eager; return the \
         LazyFrame"
    )]
    EagerCall {
        /// The path of the model.
        path: String,
        /// The line of the call.
        line: usize,
        /// The column of the call.
        column: usize,
        /// The call as found, such as `.collect()` or `pl.DataFrame()`.
        call: String,
    },
}

impl PythonModelError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::AsyncStep { .. } => "MG-0618",
            Self::InvalidReturn { .. } => "MG-0619",
            Self::UnresolvedParameter { .. } => "MG-0620",
            Self::VariadicParameter { .. } => "MG-0621",
            Self::StepCycle { .. } => "MG-0622",
            Self::MissingResult { .. } => "MG-0623",
            Self::EagerCall { .. } => "MG-0624",
            Self::PositionalOnlyParameter { .. } => "MG-0625",
            Self::PythonSyntax { .. } => "MG-0626",
        }
    }

    /// Returns the path of the model.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::AsyncStep { path, .. }
            | Self::InvalidReturn { path, .. }
            | Self::UnresolvedParameter { path, .. }
            | Self::VariadicParameter { path, .. }
            | Self::StepCycle { path, .. }
            | Self::MissingResult { path, .. }
            | Self::PositionalOnlyParameter { path, .. }
            | Self::PythonSyntax { path, .. }
            | Self::EagerCall { path, .. } => path,
        }
    }

    /// Returns the line that caused the error, when there is one.
    #[must_use]
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::AsyncStep { line, .. }
            | Self::InvalidReturn { line, .. }
            | Self::UnresolvedParameter { line, .. }
            | Self::VariadicParameter { line, .. }
            | Self::StepCycle { line, .. }
            | Self::PositionalOnlyParameter { line, .. }
            | Self::EagerCall { line, .. } => Some(*line),
            Self::PythonSyntax { line, .. } => *line,
            Self::MissingResult { .. } => None,
        }
    }

    /// Returns the column that caused the error, when there is one.
    #[must_use]
    pub fn column(&self) -> Option<usize> {
        match self {
            Self::UnresolvedParameter { column, .. }
            | Self::VariadicParameter { column, .. }
            | Self::PositionalOnlyParameter { column, .. }
            | Self::EagerCall { column, .. } => Some(*column),
            Self::PythonSyntax { column, .. } => *column,
            _ => None,
        }
    }
}

/// Formats the line and column of a syntax error, as far as Python gives them.
fn location(line: Option<usize>, column: Option<usize>) -> String {
    match (line, column) {
        (Some(line), Some(column)) => format!(" line {line} column {column}"),
        (Some(line), None) => format!(" line {line}"),
        _ => String::new(),
    }
}
