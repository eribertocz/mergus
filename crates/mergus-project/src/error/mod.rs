//! The errors of reading a project.

use std::fmt;

use mergus_ports::PortError;

mod access;
mod python;
mod warning;

pub use python::PythonModelError;
pub use warning::ProjectWarning;
mod format;

use format::{cycle_names, cycle_references, hint, key_prefix};

/// The kind of a name checked by MG-REQ-0305.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NameKind {
    /// A model: the stem of a `.sql` or `.py` file under `models/`.
    Model,
    /// A folder: a directory under `models/`, other than a schema.
    Folder,
    /// A schema: a directory directly under `models/gold/`.
    Schema,
    /// A source: the stem of a `sources/*.toml` file.
    Source,
    /// A seed: the stem of a `seeds/*.csv` file.
    Seed,
}

impl fmt::Display for NameKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Model => "model",
            Self::Folder => "folder",
            Self::Schema => "schema",
            Self::Source => "source",
            Self::Seed => "seed",
        })
    }
}

/// A problem of the project. Paths are relative to the project root, separated by `/`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectError {
    /// Two model files have the same stem (MG-REQ-0303).
    #[error("model `{name}` is defined twice: `{first}` and `{second}`")]
    DuplicateModel {
        /// The model name.
        name: String,
        /// The path of the first file, in walking order.
        first: String,
        /// The path of the later file.
        second: String,
    },
    /// A name is not a lowercase identifier (MG-REQ-0305).
    #[error("{kind} name `{name}` of `{path}` is not a lowercase identifier; use `{suggestion}`")]
    InvalidName {
        /// What the name names.
        kind: NameKind,
        /// The name given.
        name: String,
        /// The path of the file or directory that carries the name.
        path: String,
        /// The valid name of MG-REQ-0360.
        suggestion: String,
    },
    /// A configuration file is not TOML 1.0 (MG-REQ-0316).
    #[error("`{path}` line {line} is not TOML 1.0: {message}")]
    InvalidToml {
        /// The path of the file.
        path: String,
        /// The line of the problem, from 1.
        line: usize,
        /// What the parser found.
        message: String,
    },
    /// A key is not in the schema of its file (MG-REQ-0316, MG-REQ-0364).
    #[error("`{path}` line {line}: unknown key `{key}`{}", hint(suggestion.as_deref()))]
    UnknownKey {
        /// The path of the file.
        path: String,
        /// The line of the key, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
        /// The closest key allowed at that place (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A value has the wrong type, is invalid, or a required key is missing (MG-REQ-0316).
    #[error("`{path}` line {line}: key `{key}`: {message}{}", hint(suggestion.as_deref()))]
    InvalidValue {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
        /// What the schema expects.
        message: String,
        /// The closest allowed value (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A string value contains `${`, `{{` or `{%` (MG-REQ-0364).
    #[error(
        "`{path}` line {line}: key `{key}` holds an expression; configuration values are literals"
    )]
    Expression {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
    },
    /// A string value is a literal secret (MG-REQ-0320, MG-REQ-0363). The value is never shown.
    #[error(
        "`{path}` line {line}: key `{key}` holds a literal secret; write a reference \
         `secret:<name>` instead"
    )]
    LiteralSecret {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
    },
    /// A float is NaN or infinite, which JSON cannot hold (MG-REQ-0365).
    #[error("`{path}` line {line}: key `{key}` is NaN or infinite")]
    NonFiniteFloat {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
    },
    /// A section of a folder `mergus.toml` names no model file of its folder (MG-REQ-0331).
    #[error(
        "`{path}` line {line}: section `{section}` names no model of this folder{}",
        hint(suggestion.as_deref())
    )]
    UnknownSection {
        /// The path of the file.
        path: String,
        /// The line of the section, from 1.
        line: usize,
        /// The section name.
        section: String,
        /// The closest model name of the folder (MG-REQ-0331).
        suggestion: Option<String>,
    },
    /// `ids.toml` is not TOML 1.0, or has a table, key or value that MG-REQ-0370.1 does not
    /// allow.
    #[error("`{path}` line {line}: {}{message}", key_prefix(key.as_deref()))]
    InvalidIds {
        /// The path of the file.
        path: String,
        /// The line of the problem, from 1.
        line: usize,
        /// The key, dotted from the root of the file, when the problem has one.
        key: Option<String>,
        /// What is wrong.
        message: String,
    },
    /// Two entries of `ids.toml` have the same identifier (MG-REQ-0370.3).
    #[error(
        "`{path}` line {line}: identifier `{identifier}` is given to both `{first}` and `{second}`"
    )]
    DuplicateIdsEntry {
        /// The path of the file.
        path: String,
        /// The line of the later entry, from 1.
        line: usize,
        /// The identifier.
        identifier: String,
        /// The dotted key of the earlier entry.
        first: String,
        /// The dotted key of the later entry.
        second: String,
    },
    /// Two objects resolve to the same identifier (MG-REQ-0371).
    #[error(
        "{first} (`{first_path}`) and {second} (`{path}`) have the same identifier `{identifier}`"
    )]
    DuplicateIdentifier {
        /// The identifier.
        identifier: String,
        /// The earlier object.
        first: String,
        /// The path of the file that declares the earlier object.
        first_path: String,
        /// The later object.
        second: String,
        /// The path of the file that declares the later object.
        path: String,
    },
    /// A `.sql` model reads a table the project does not know (MG-REQ-0307, MG-REQ-0373).
    #[error(
        "`{path}` line {line} column {column}: `{name}` is not a model, source table or seed \
         of the project{}",
        hint(suggestion.as_deref())
    )]
    UnknownReference {
        /// The path of the model.
        path: String,
        /// The line of the reference, from 1.
        line: usize,
        /// The column of the reference, from 1, in Unicode scalar values.
        column: usize,
        /// The reference as written, its parts joined by `.`.
        name: String,
        /// The closest known table (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A `.sql` model is not one read-only `SELECT` (MG-REQ-0308, MG-REQ-0374).
    #[error(
        "`{path}` line {line} column {column}: {construct} is not allowed; a model is exactly \
         one read-only SELECT"
    )]
    ForbiddenSql {
        /// The path of the model.
        path: String,
        /// The line of the construct, from 1.
        line: usize,
        /// The column of the construct, from 1, in Unicode scalar values.
        column: usize,
        /// The rejected construct.
        construct: String,
    },
    /// The references of `.sql` models form a cycle (MG-REQ-0375).
    #[error(
        "models {} form a cycle: {}",
        cycle_names(models),
        cycle_references(models, references)
    )]
    ModelCycle {
        /// Every model of the cycle in reference order, from the first in name order.
        models: Vec<String>,
        /// For each model of `models`, the path of its file and the line, from 1, of its
        /// reference to the next model of the cycle.
        references: Vec<(String, usize)>,
    },
    /// DuckDB refuses to bind a `.sql` model against its inputs (MG-REQ-0604).
    #[error("`{path}`: DuckDB cannot prepare the model: {message}")]
    UnpreparableSql {
        /// The path of the model.
        path: String,
        /// What DuckDB reports.
        message: String,
    },
    /// A `.sql` model names a parameter that is not a run parameter (MG-REQ-0603).
    #[error(
        "`{path}` line {line} column {column}: parameter `${name}` is not a run parameter; use \
         `$run_date`, `$period_start` or `$period_end`"
    )]
    UnknownParameter {
        /// The path of the model.
        path: String,
        /// The line of the parameter, from 1.
        line: usize,
        /// The column of the parameter, from 1.
        column: usize,
        /// The name of the parameter, without its `$`.
        name: String,
    },
    /// A `.sql` model has a positional parameter (MG-REQ-0603).
    #[error(
        "`{path}` line {line} column {column}: parameter `{written}` is positional; run \
         parameters are named: `$run_date`, `$period_start` or `$period_end`"
    )]
    PositionalParameter {
        /// The path of the model.
        path: String,
        /// The line of the parameter, from 1.
        line: usize,
        /// The column of the parameter, from 1.
        column: usize,
        /// The parameter as written, such as `?` or `$1`.
        written: String,
    },
    /// A macro file holds a statement DuckDB refuses, or creates an object other than a
    /// macro (MG-REQ-0641.2).
    #[error("`{path}`: {reason}; a macro file holds only CREATE MACRO statements")]
    ForbiddenMacroFile {
        /// The path of the macro file.
        path: String,
        /// DuckDB's refusal, or the objects the file creates.
        reason: String,
    },
    /// Two macros have one name, compared without case (MG-REQ-0642).
    #[error("macro `{name}` is defined twice: `{first}` and `{path}`")]
    DuplicateMacro {
        /// The name of the later macro.
        name: String,
        /// The path of the earlier file, in the sorted order of `macros/`.
        first: String,
        /// The path of the later file.
        path: String,
    },
    /// A macro has the name of a DuckDB built-in function or of a macro of Mergus, such as
    /// `as_of`, compared without case (MG-REQ-0642).
    #[error("`{path}`: macro `{name}` shadows the built-in function `{builtin}`")]
    MacroShadowsBuiltin {
        /// The name of the macro.
        name: String,
        /// The path of its file.
        path: String,
        /// The name of the built-in, as DuckDB or Mergus gives it.
        builtin: String,
    },
    /// A `seeds/<name>.csv` has no `[seeds.<name>]` in `project.toml` (MG-REQ-0645.2).
    #[error(
        "seed `{path}` is not declared; add `[seeds.{name}]` with its column types to \
         `project.toml`"
    )]
    UndeclaredSeed {
        /// The seed name.
        name: String,
        /// The path of the CSV file.
        path: String,
    },
    /// A `[seeds.<name>]` of `project.toml` has no `seeds/<name>.csv` (MG-REQ-0645.2).
    #[error("`{path}` line {line}: seed `{name}` is declared but `seeds/{name}.csv` is missing")]
    SeedWithoutFile {
        /// The seed name.
        name: String,
        /// The path of `project.toml`.
        path: String,
        /// The line of the declaration, from 1.
        line: usize,
    },
    /// A `[seeds.<name>]` has a key other than `columns`, `delimiter` and `header`
    /// (MG-REQ-0645.1, MG-REQ-0645.2).
    #[error("`{path}` line {line}: unknown seed key `{key}`{}", hint(suggestion.as_deref()))]
    UnknownSeedKey {
        /// The path of `project.toml`.
        path: String,
        /// The line of the key, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
        /// The closest allowed key (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A seed column has a type that is not a DuckDB column type name as `DESCRIBE` writes
    /// it (MG-REQ-0645.1, MG-REQ-0645.2).
    #[error(
        "`{path}` line {line}: key `{key}`: `{type_name}` is not a DuckDB column type as \
         DESCRIBE writes it, such as `BIGINT` or `VARCHAR`"
    )]
    UnknownSeedType {
        /// The path of `project.toml`.
        path: String,
        /// The line of the column, from 1.
        line: usize,
        /// The key of the column, dotted from the root of the file.
        key: String,
        /// The type given.
        type_name: String,
    },
    /// A problem of a `.py` model (MG-REQ-0612 to MG-REQ-0616).
    #[error(transparent)]
    Python(#[from] PythonModelError),
}

/// Why a project layout could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The file system port failed.
    #[error(transparent)]
    Port(#[from] PortError),
    /// A bundled configuration schema cannot be compiled: a defect of Mergus itself.
    #[error("configuration schema `{0}` is invalid")]
    Schema(&'static str),
    /// The project has problems, every one of them, in walking order.
    #[error("the project has {} problem(s)", .0.len())]
    Invalid(Vec<ProjectError>),
}
