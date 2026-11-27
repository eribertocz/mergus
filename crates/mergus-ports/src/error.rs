//! The error every port returns.

use std::path::PathBuf;
use std::time::Duration;

/// A failure reported by a port.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    /// The file, object, source, host or program does not exist.
    #[error("`{0}` was not found")]
    NotFound(String),
    /// The operation failed.
    #[error("{0}")]
    Failed(String),
    /// DuckDB refused a statement: a parser, catalog or binder error in its text, which the
    /// user can fix. The message is DuckDB's.
    #[error("{0}")]
    Refused(String),
    /// A column type that is not a DuckDB scalar type Mergus writes into DDL.
    #[error("`{0}` is not a DuckDB column type Mergus can create")]
    UnsupportedType(String),
    /// A DuckDB extension is not in its cache directory and cannot be downloaded from its
    /// host (MG-REQ-0194.3).
    #[error(
        "the DuckDB extension `{extension}` is not in `{}` and cannot be downloaded from `{host}`",
        cache_dir.display()
    )]
    ExtensionUnavailable {
        /// The name of the extension.
        extension: String,
        /// The host it is downloaded from.
        host: String,
        /// The directory it is cached in.
        cache_dir: PathBuf,
    },
    /// A DuckLake catalog attached with the `ENCRYPTED` option exists and is not encrypted
    /// (MG-REQ-0515.3).
    #[error("the DuckLake catalog `{catalog}` is not encrypted")]
    UnencryptedCatalog {
        /// The metadata database of the catalog, as attached.
        catalog: String,
    },
    /// The connection value of a PostgreSQL source already sets a libpq parameter that
    /// Mergus sets for a protected session (MG-REQ-0429.3). The value is never shown.
    #[error(
        "the connection value of source `{source_name}` already sets `{parameter}`, which Mergus \
         sets for every source session"
    )]
    SessionParameterSet {
        /// The name the source is attached as.
        source_name: String,
        /// The parameter, `application_name` or `options`.
        parameter: String,
    },
    /// No Python interpreter was found: neither the configured one, nor one in the project's
    /// `.venv`, nor one on the search path. `searched` lists what was tried, in order.
    #[error("no Python interpreter was found; tried {}", .searched.join(", "))]
    PythonNotFound {
        /// The interpreters tried, in order.
        searched: Vec<String>,
    },
    /// A program did not finish in time and was killed.
    #[error("`{program}` did not finish within {after:?} and was stopped")]
    TimedOut {
        /// The program.
        program: String,
        /// The limit it reached.
        after: Duration,
    },
    /// A Python file is not valid Python. The message is the interpreter's; lines and columns
    /// start at 1, and a column counts characters.
    #[error("{message}")]
    PythonSyntax {
        /// The interpreter's message.
        message: String,
        /// The line of the error, when the interpreter gives one.
        line: Option<u32>,
        /// The column of the error, when the interpreter gives one.
        column: Option<u32>,
    },
}

impl PortError {
    /// Returns the `MG-####` code of an error the user can fix in the project
    /// (MG-REQ-0008); `None` for the others, which the caller reports with its own code.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::SessionParameterSet { .. } => Some("MG-0410"),
            Self::NotFound(_)
            | Self::Failed(_)
            | Self::Refused(_)
            | Self::UnsupportedType(_)
            | Self::ExtensionUnavailable { .. }
            | Self::UnencryptedCatalog { .. }
            | Self::PythonNotFound { .. }
            | Self::TimedOut { .. }
            | Self::PythonSyntax { .. } => None,
        }
    }
}
