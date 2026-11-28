//! The errors of the binary itself, each with its `MG-13xx` code, or the `MG-07xx` code of
//! `run` and `backfill` (MG-REQ-0008).

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
    #[error("unexpected argument `{given}`: `{command}` takes no more arguments")]
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
    /// An option is given without its value.
    #[error("option `{option}` needs a value")]
    MissingValue {
        /// The option.
        option: &'static str,
    },
    /// An option that may be given once is given twice.
    #[error("option `{option}` is given more than once")]
    RepeatedOption {
        /// The option.
        option: &'static str,
    },
    /// An argument is given a value it does not allow.
    #[error("`{given}` is not a value of `{argument}`")]
    InvalidValue {
        /// The argument.
        argument: &'static str,
        /// The value given.
        given: String,
        /// The closest allowed value (MG-REQ-0359).
        closest: &'static str,
    },
    /// `secret set` is given a word it does not take, which may be the value; the word is
    /// never shown (MG-REQ-1007.5).
    #[error(
        "`secret set` takes only the name of the secret; the value is read from standard input"
    )]
    SecretArgument,
    /// A required argument is missing.
    #[error("`{command}` needs the argument `{argument}`")]
    MissingArgument {
        /// The argument.
        argument: &'static str,
        /// The command.
        command: &'static str,
    },
    /// A name given to `init`, `new model` or `add source` is not a lowercase identifier
    /// (MG-REQ-0305, MG-REQ-1350.3).
    #[error("{kind} name `{name}` is not a lowercase identifier; use `{suggestion}`")]
    InvalidName {
        /// What the name names: `project`, `model`, `schema` or `source`.
        kind: &'static str,
        /// The name given.
        name: String,
        /// The valid name of MG-REQ-0360.
        suggestion: String,
    },
    /// The name of a new model is the name of another model (MG-REQ-0303, MG-REQ-1350.3).
    #[error("model name `{name}` is taken by `{path}`")]
    NameTaken {
        /// The name given.
        name: String,
        /// The path of the model that has it, relative to the project root.
        path: String,
    },
    /// A file the command creates already exists (MG-REQ-1349.2, MG-REQ-1350.3).
    #[error("`{path}` already exists")]
    FileExists {
        /// The path of the file, relative to the folder the command writes in.
        path: String,
    },
    /// A file or a folder cannot be written.
    #[error("`{path}` cannot be written: {message}")]
    Unwritable {
        /// The path, relative to the folder the command writes in.
        path: String,
        /// What the file system said.
        message: String,
    },
    /// No directory from the current one upwards holds `project.toml` (MG-REQ-1344).
    #[error("no `project.toml` in `{start}` or in any directory above it")]
    ProjectNotFound {
        /// The directory the search started in.
        start: String,
    },
    /// A word of a selection is not a selector (MG-REQ-1305).
    #[error("`{given}` is not a selector: {reason}")]
    InvalidSelector {
        /// The word given.
        given: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// A selector or `explain` names a model or a source the project does not have
    /// (MG-REQ-1305, MG-REQ-1352.3).
    #[error("{kind} `{given}` does not exist{}", closest_suffix(.closest.as_deref()))]
    UnknownName {
        /// What the name names: `model` or `source`.
        kind: &'static str,
        /// The name given.
        given: String,
        /// The closest name of that kind (MG-REQ-0359).
        closest: Option<String>,
    },
    /// `explain` names a code the catalog does not have (MG-REQ-1352.3).
    #[error("error code `{given}` does not exist{}", closest_suffix(.closest.as_deref()))]
    UnknownCode {
        /// The code given.
        given: String,
        /// The closest code of the catalog (MG-REQ-0359).
        closest: Option<&'static str>,
    },
    /// A setting has a value it does not allow (MG-REQ-1351).
    #[error("{origin} sets `max_parallel` to `{given}`, which is not an integer from 1")]
    InvalidSetting {
        /// Where the value comes from.
        origin: String,
        /// The value given.
        given: String,
    },
    /// The user configuration file is not TOML or has an unknown key (MG-REQ-1351).
    #[error("`{path}` is not valid: {message}")]
    InvalidUserConfig {
        /// The path of the file.
        path: String,
        /// What is wrong.
        message: String,
    },
    /// An idempotency key is given again, within its 24 hours, with other arguments
    /// (MG-REQ-1313).
    #[error("idempotency key `{key}` was used with other arguments")]
    IdempotencyConflict {
        /// The key given.
        key: String,
    },
    /// `--from` or `--to` of `backfill` is not a date (MG-REQ-0710, MG-REQ-0657).
    #[error("`{option}` is `{given}`, which is not a date `YYYY-MM-DD`")]
    InvalidDate {
        /// The option.
        option: &'static str,
        /// The value given.
        given: String,
    },
    /// The range of `backfill` ends before it starts (MG-REQ-0657.1).
    #[error("the backfill ends at `{to}`, before it starts at `{from}`")]
    InvertedBackfill {
        /// `--from`.
        from: String,
        /// `--to`.
        to: String,
    },
    /// `run` and `backfill` need DuckDB, which needs a user cache directory for its
    /// extensions (MG-REQ-0710).
    #[error("no DuckDB is available: there is no user cache directory for its extensions")]
    NoDatabase,
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
            | Self::MissingCommand { .. }
            | Self::MissingValue { .. }
            | Self::RepeatedOption { .. }
            | Self::InvalidValue { .. }
            | Self::SecretArgument
            | Self::MissingArgument { .. } => "MG-1301",
            Self::ProjectNotFound { .. } => "MG-1302",
            Self::Unreadable(_) => "MG-1303",
            Self::Internal(_) => "MG-1304",
            Self::InvalidName { .. } => "MG-1305",
            Self::NameTaken { .. } => "MG-1306",
            Self::FileExists { .. } => "MG-1307",
            Self::Unwritable { .. } => "MG-1308",
            Self::InvalidSelector { .. } => "MG-1309",
            Self::UnknownName { .. } => "MG-1310",
            Self::UnknownCode { .. } => "MG-1311",
            Self::InvalidSetting { .. } => "MG-1312",
            Self::InvalidUserConfig { .. } => "MG-1313",
            Self::IdempotencyConflict { .. } => "MG-1314",
            Self::InvalidDate { .. } | Self::InvertedBackfill { .. } => "MG-0705",
            Self::NoDatabase => "MG-0706",
        }
    }

    /// Returns the hint of the error when it is more precise than the catalog's: for an
    /// invalid command line, the valid word to use (MG-REQ-0297).
    pub(crate) fn hint(&self) -> Option<String> {
        match self {
            Self::UnknownCommand { closest, .. }
            | Self::UnknownOption { closest, .. }
            | Self::MissingCommand { closest }
            | Self::InvalidValue { closest, .. } => Some(format!("Use `{closest}`.")),
            Self::InvalidName { suggestion, .. } => Some(format!("Use `{suggestion}`.")),
            Self::UnknownName {
                closest: Some(closest),
                ..
            } => Some(format!("Use `{closest}`.")),
            Self::UnknownCode {
                closest: Some(closest),
                ..
            } => Some(format!("Use `{closest}`.")),
            Self::MissingValue { option } => Some(format!("Write the value after `{option}`.")),
            Self::RepeatedOption { option } => Some(format!("Give `{option}` once.")),
            Self::MissingArgument {
                command: "schema", ..
            } => Some("Name a command, or give `--all`.".to_owned()),
            Self::MissingArgument { argument, .. } => Some(format!("Give `{argument}`.")),
            Self::UnexpectedArgument { given, .. } => Some(format!("Remove `{given}`.")),
            Self::SecretArgument => {
                Some("Give only the name, and write the value on standard input.".to_owned())
            }
            Self::ConflictingOptions { first, second } => {
                Some(format!("Remove `{first}` or `{second}`."))
            }
            Self::ProjectNotFound { .. }
            | Self::Unreadable(_)
            | Self::Internal(_)
            | Self::NameTaken { .. }
            | Self::FileExists { .. }
            | Self::Unwritable { .. }
            | Self::InvalidSelector { .. }
            | Self::UnknownName { closest: None, .. }
            | Self::UnknownCode { closest: None, .. }
            | Self::InvalidSetting { .. }
            | Self::InvalidUserConfig { .. }
            | Self::IdempotencyConflict { .. }
            | Self::InvalidDate { .. }
            | Self::InvertedBackfill { .. }
            | Self::NoDatabase => None,
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

/// `; did you mean` and the closest name in backquotes when one exists, otherwise nothing.
fn closest_suffix(closest: Option<&str>) -> String {
    closest.map_or_else(String::new, |closest| {
        format!("; did you mean `{closest}`?")
    })
}
