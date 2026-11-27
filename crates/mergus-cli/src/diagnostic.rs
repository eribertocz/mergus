//! Diagnostics: every error found, with its code, message, location, hint and next steps
//! (MG-REQ-1324, MG-REQ-1334).

use mergus_project::ProjectError;
use mergus_runtime::StorageProblem;
use mergus_security::{SecretError, StoreError};
use mergus_source::{IndexWarning, Severity, SourceProblem};

use crate::catalog::{self, Entry};
use crate::error::CliError;

/// Where the cause of a diagnostic is: a file relative to the project root, with `/`, then a
/// line and a column from 1 when they are known (MG-REQ-0296.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Location {
    pub(crate) file: String,
    pub(crate) line: Option<usize>,
    pub(crate) column: Option<usize>,
}

/// One error or warning found by a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub(crate) code: &'static str,
    /// Whether it is a warning, which does not fail the command (MG-REQ-0416).
    pub(crate) warning: bool,
    pub(crate) message: String,
    pub(crate) location: Option<Location>,
    /// The hint, when it is more precise than the catalog's.
    pub(crate) hint: Option<String>,
    pub(crate) next: Vec<Vec<String>>,
    /// The text of the line of the location, when the file could be read (MG-REQ-1346.1).
    pub(crate) source: Option<String>,
}

/// What to do when the keychain of this platform cannot be written (MG-REQ-1007.4).
const KEYCHAIN_HINT: &str = if cfg!(target_os = "macos") {
    "Unlock the macOS Keychain, then run the command again."
} else if cfg!(windows) {
    "Make sure the Windows Credential Manager service is running, then run the command again."
} else {
    "Start a Secret Service, such as GNOME Keyring or KWallet, then run the command again."
};

/// A diagnostic with what the catalog says about its code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Resolved<'a> {
    pub(crate) diagnostic: &'a Diagnostic,
    pub(crate) hint: String,
    /// Why the error matters: the cause of the catalog (MG-REQ-1346.3).
    pub(crate) why: String,
    pub(crate) exit_code: u8,
}

impl Diagnostic {
    /// The diagnostic of a problem of the project, located at its file and, when the problem
    /// names one, its line.
    pub(crate) fn of_project(error: &ProjectError) -> Self {
        Self {
            code: error.code(),
            warning: false,
            message: error.to_string(),
            location: Some(Location {
                file: error.path().to_owned(),
                line: error.line(),
                column: None,
            }),
            hint: project_hint(error.code()).map(str::to_owned),
            next: Vec::new(),
            source: None,
        }
    }

    /// The diagnostic of a problem of a source file of `mergus-source`, located at its file
    /// and line; a warning when the problem is one.
    pub(crate) fn of_source(problem: &SourceProblem) -> Self {
        Self {
            code: problem.code(),
            warning: problem.severity() == Severity::Warning,
            message: problem.to_string(),
            location: Some(Location {
                file: problem.path().to_owned(),
                line: Some(problem.line()),
                column: None,
            }),
            hint: None,
            next: Vec::new(),
            source: None,
        }
    }

    /// The diagnostic of a problem of the storage of the zones of `mergus-runtime`, located
    /// at its file and line (MG-REQ-0501).
    pub(crate) fn of_storage(problem: &StorageProblem) -> Self {
        Self {
            code: problem.code(),
            warning: false,
            message: problem.to_string(),
            location: Some(Location {
                file: problem.path().to_owned(),
                line: Some(problem.line()),
                column: None,
            }),
            hint: None,
            next: Vec::new(),
            source: None,
        }
    }

    /// The diagnostic of a secret value `secret set` did not store. A keychain that cannot be
    /// written gets the hint of the keychain service of the platform (MG-REQ-1007.4).
    pub(crate) fn of_store(error: &StoreError) -> Self {
        let hint = match error {
            StoreError::Unwritable { .. } => Some(KEYCHAIN_HINT.to_owned()),
            StoreError::Empty { .. } => None,
        };
        Self {
            code: error.code(),
            warning: false,
            message: error.to_string(),
            location: None,
            hint,
            next: Vec::new(),
            source: None,
        }
    }

    /// The diagnostic of a secret reference that does not resolve, for `doctor`: its fix is
    /// `mergus secret set <name>` (MG-REQ-1004.3).
    pub(crate) fn of_secret(error: &SecretError) -> Self {
        let name = error.reference().name();
        let command = vec![
            crate::name::PRODUCT.to_owned(),
            "secret".to_owned(),
            "set".to_owned(),
            name.to_owned(),
        ];
        Self {
            code: error.code(),
            warning: false,
            message: error.to_string(),
            location: None,
            hint: Some(format!("Run `{}`.", command.join(" "))),
            next: vec![command],
            source: None,
        }
    }

    /// The diagnostic of the error `code` of another crate, located nowhere.
    pub(crate) fn of_code(code: &'static str, message: String) -> Self {
        Self {
            code,
            warning: false,
            message,
            location: None,
            hint: None,
            next: Vec::new(),
            source: None,
        }
    }

    /// The warning `code` of another crate, located nowhere: it does not fail the command.
    pub(crate) fn warning_of_code(code: &'static str, message: String) -> Self {
        Self {
            warning: true,
            ..Self::of_code(code, message)
        }
    }

    /// The warning of a key or incremental column without an index, located at the source
    /// file `path`, with the index to create as its hint (MG-REQ-0431).
    pub(crate) fn of_index(warning: &IndexWarning, path: &str) -> Self {
        Self {
            code: warning.code(),
            warning: true,
            message: warning.to_string(),
            location: Some(Location {
                file: path.to_owned(),
                line: None,
                column: None,
            }),
            hint: Some(format!(
                "Add the index `{}` to the source.",
                warning.suggestion()
            )),
            next: Vec::new(),
            source: None,
        }
    }

    /// The diagnostic of an error of the binary.
    pub(crate) fn of_cli(error: &CliError) -> Self {
        Self {
            code: error.code(),
            warning: false,
            message: error.to_string(),
            location: None,
            hint: error.hint(),
            next: error.next(),
            source: None,
        }
    }

    /// Adds the hint and the exit code of the catalog.
    ///
    /// # Errors
    ///
    /// Fails with [`CliError::Internal`] when the code has no catalog entry.
    pub(crate) fn resolve(&self) -> Result<Resolved<'_>, CliError> {
        let Entry {
            hint,
            exit_code,
            cause,
            ..
        } = catalog::entry(self.code).ok_or_else(|| {
            CliError::Internal(format!("{} has no entry in the error catalog", self.code))
        })?;
        Ok(Resolved {
            diagnostic: self,
            hint: self.hint.clone().unwrap_or(hint),
            why: cause,
            exit_code,
        })
    }
}

/// Orders `diagnostics` by file, the files in the order they first appear, keeping the order
/// within a file; diagnostics without a location come last (MG-REQ-1323).
pub(crate) fn group(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut files: Vec<Option<String>> = Vec::new();
    for diagnostic in &diagnostics {
        let file = diagnostic.location.as_ref().map(|at| at.file.clone());
        if file.is_some() && !files.contains(&file) {
            files.push(file);
        }
    }
    files.push(None);
    let mut grouped = Vec::with_capacity(diagnostics.len());
    for file in &files {
        grouped.extend(
            diagnostics
                .iter()
                .filter(|d| d.location.as_ref().map(|at| &at.file) == file.as_ref())
                .cloned(),
        );
    }
    grouped
}

/// The hint of a project problem whose catalog `Fix` starts with an example, which would
/// read as if it named the file at fault.
fn project_hint(code: &str) -> Option<&'static str> {
    match code {
        "MG-0301" => Some("Rename one of the two files so that every model name is unique."),
        "MG-0302" => Some("Rename it to the valid name the message suggests."),
        "MG-0309" => {
            Some("Store the value in a secret backend and reference it as `secret:<name>`.")
        }
        _ => None,
    }
}
