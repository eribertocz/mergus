//! The problems of a source file.

use std::fmt;

use crate::Mode;

/// Whether a problem fails validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Validation fails.
    Error,
    /// Validation passes; the problem is reported.
    Warning,
}

/// A problem of a source file. Paths are relative to the project root, separated by `/`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SourceProblem {
    /// An incremental table declares no `key` (MG-REQ-0414).
    #[error(
        "`{path}` line {line}: table `{table}` of source `{source_name}` is incremental but \
         declares no `key`"
    )]
    IncrementalWithoutKey {
        /// The path of the source file.
        path: String,
        /// The line of `incremental`, from 1.
        line: usize,
        /// The source name.
        source_name: String,
        /// The table name.
        table: String,
    },
    /// A `connect` or `live` source declares no `engine` or one its mode does not allow, or a
    /// `copy` source declares one (MG-REQ-0409).
    #[error("`{path}` line {line}: source `{source_name}` {}", engine_problem(*mode, engine.as_deref()))]
    EngineNotAllowed {
        /// The path of the source file.
        path: String,
        /// The line of `engine`, or of `mode` when there is no `engine`, from 1.
        line: usize,
        /// The source name.
        source_name: String,
        /// The mode of the source.
        mode: Mode,
        /// The declared engine, if any.
        engine: Option<String>,
    },
    /// An incremental table declares no `deletes`: deletes in the source will not be seen
    /// (MG-REQ-0416). A warning.
    #[error(
        "`{path}` line {line}: table `{table}` of source `{source_name}` is incremental but \
         declares no `deletes`; rows deleted in the source will not be seen"
    )]
    NoDeletePolicy {
        /// The path of the source file.
        path: String,
        /// The line of `incremental`, from 1.
        line: usize,
        /// The source name.
        source_name: String,
        /// The table name.
        table: String,
    },
}

/// What is wrong with the engine of a source of `mode`.
fn engine_problem(mode: Mode, engine: Option<&str>) -> String {
    match (mode, engine) {
        (Mode::Copy, _) => {
            "declares `engine`, which only `connect` and `live` sources may declare".to_owned()
        }
        (_, None) => format!(
            "is read in `{mode}` mode but declares no `engine`; `{mode}` allows {}",
            allowed(mode)
        ),
        (_, Some(engine)) => format!(
            "is read in `{mode}` mode on engine `{engine}`, which it does not allow; `{mode}` \
             allows {}",
            allowed(mode)
        ),
    }
}

/// The engines of `mode`, quoted and joined.
fn allowed(mode: Mode) -> String {
    let names: Vec<String> = mode
        .engines()
        .iter()
        .map(|name| format!("`{name}`"))
        .collect();
    match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    }
}

impl SourceProblem {
    /// Returns the `MG-####` code of the problem (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::IncrementalWithoutKey { .. } => "MG-0401",
            Self::EngineNotAllowed { .. } => "MG-0402",
            Self::NoDeletePolicy { .. } => "MG-0403",
        }
    }

    /// Returns whether the problem fails validation.
    #[must_use]
    pub fn severity(&self) -> Severity {
        match self {
            Self::IncrementalWithoutKey { .. } | Self::EngineNotAllowed { .. } => Severity::Error,
            Self::NoDeletePolicy { .. } => Severity::Warning,
        }
    }

    /// Returns the path of the source file, relative to the project root.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::IncrementalWithoutKey { path, .. }
            | Self::EngineNotAllowed { path, .. }
            | Self::NoDeletePolicy { path, .. } => path,
        }
    }

    /// Returns the line of the source file that caused the problem, from 1.
    #[must_use]
    pub fn line(&self) -> usize {
        match self {
            Self::IncrementalWithoutKey { line, .. }
            | Self::EngineNotAllowed { line, .. }
            | Self::NoDeletePolicy { line, .. } => *line,
        }
    }

    /// Returns the key that caused the problem, dotted from the root of the file.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::IncrementalWithoutKey { table, .. } | Self::NoDeletePolicy { table, .. } => {
                format!("tables.{table}.incremental")
            }
            Self::EngineNotAllowed { engine: None, .. } => "mode".to_owned(),
            Self::EngineNotAllowed { .. } => "engine".to_owned(),
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}
