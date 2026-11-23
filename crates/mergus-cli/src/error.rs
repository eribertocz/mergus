//! The errors of the binary itself, each with its `MG-13xx` code (MG-REQ-0008).

/// A failure of the binary, outside the checks of the crates it calls.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CliError {
    /// The command line names an unknown command (MG-REQ-0297).
    #[error("unknown command `{given}`; did you mean `{closest}`?")]
    UnknownCommand {
        /// The word given.
        given: String,
        /// The closest command (MG-REQ-0359).
        closest: &'static str,
    },
    /// The command line names an unknown option, or gives a value to one that takes none.
    #[error("unknown option `{given}`; did you mean `{closest}`?")]
    UnknownOption {
        /// The word given.
        given: String,
        /// The closest option (MG-REQ-0359).
        closest: &'static str,
    },
    /// The command line gives an argument the command does not take.
    #[error("unexpected argument `{given}`: `{command}` takes no argument")]
    UnexpectedArgument {
        /// The argument given.
        given: String,
        /// The command.
        command: &'static str,
    },
    /// The command line asks for two options that exclude each other (MG-REQ-1347.4).
    #[error("`{first}` and `{second}` cannot be used together")]
    ConflictingOptions {
        /// The first option.
        first: &'static str,
        /// The second option.
        second: &'static str,
    },
    /// The command line names no command.
    #[error("no command given; did you mean `{closest}`?")]
    MissingCommand {
        /// The command to suggest.
        closest: &'static str,
    },
    /// No directory from the current one upwards holds `project.toml` (MG-REQ-1344).
    #[error("no `project.toml` in `{start}` or in any directory above it")]
    ProjectNotFound {
        /// The directory the search started in.
        start: String,
    },
    /// A file or directory of the project cannot be read.
    #[error("the project cannot be read: {0}")]
    Unreadable(String),
    /// A defect of Mergus itself.
    #[error("internal error: {0}")]
    Internal(String),
}

impl CliError {
    /// Returns the `MG-####` code of the error.
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::UnknownCommand { .. }
            | Self::UnknownOption { .. }
            | Self::UnexpectedArgument { .. }
            | Self::ConflictingOptions { .. }
            | Self::MissingCommand { .. } => "MG-1301",
            Self::ProjectNotFound { .. } => "MG-1302",
            Self::Unreadable(_) => "MG-1303",
            Self::Internal(_) => "MG-1304",
        }
    }

    /// Returns the hint of the error when it is more precise than the catalog's: for an
    /// invalid command line, the valid word to use (MG-REQ-0297).
    pub(crate) fn hint(&self) -> Option<String> {
        match self {
            Self::UnknownCommand { closest, .. }
            | Self::UnknownOption { closest, .. }
            | Self::MissingCommand { closest } => Some(format!("Use `{closest}`.")),
            Self::UnexpectedArgument { given, .. } => Some(format!("Remove `{given}`.")),
            Self::ConflictingOptions { first, second } => {
                Some(format!("Remove `{first}` or `{second}`."))
            }
            Self::ProjectNotFound { .. } | Self::Unreadable(_) | Self::Internal(_) => None,
        }
    }

    /// Returns the invocations that address the error (MG-REQ-1334).
    pub(crate) fn next(&self) -> Vec<Vec<String>> {
        match self {
            Self::ProjectNotFound { .. } => {
                vec![vec![crate::name::PRODUCT.to_owned(), "init".to_owned()]]
            }
            _ => Vec::new(),
        }
    }
}
