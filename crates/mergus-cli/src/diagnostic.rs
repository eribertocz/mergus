//! Diagnostics: every error found, with its code, message, location, hint and next steps
//! (MG-REQ-1324, MG-REQ-1334).

use mergus_project::ProjectError;

use crate::catalog::{self, Entry};
use crate::error::CliError;

/// Where the cause of a diagnostic is: a file relative to the project root, with `/`, and a
/// line from 1 (MG-REQ-0296.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Location {
    pub(crate) file: String,
    pub(crate) line: usize,
}

/// One error found by a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) location: Option<Location>,
    /// The hint, when it is more precise than the catalog's.
    hint: Option<String>,
    pub(crate) next: Vec<Vec<String>>,
}

/// A diagnostic with what the catalog says about its code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Resolved<'a> {
    pub(crate) diagnostic: &'a Diagnostic,
    pub(crate) hint: String,
    pub(crate) exit_code: u8,
}

impl Diagnostic {
    /// The diagnostic of a problem of the project. Its location is absent when the problem
    /// names no line.
    pub(crate) fn of_project(error: &ProjectError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
            location: error.line().map(|line| Location {
                file: error.path().to_owned(),
                line,
            }),
            hint: project_hint(error.code()).map(str::to_owned),
            next: Vec::new(),
        }
    }

    /// The diagnostic of an error of the binary.
    pub(crate) fn of_cli(error: &CliError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
            location: None,
            hint: error.hint(),
            next: error.next(),
        }
    }

    /// Adds the hint and the exit code of the catalog.
    ///
    /// # Errors
    ///
    /// Fails with [`CliError::Internal`] when the code has no catalog entry.
    pub(crate) fn resolve(&self) -> Result<Resolved<'_>, CliError> {
        let Entry {
            hint, exit_code, ..
        } = catalog::entry(self.code).ok_or_else(|| {
            CliError::Internal(format!("{} has no entry in the error catalog", self.code))
        })?;
        Ok(Resolved {
            diagnostic: self,
            hint: self.hint.clone().unwrap_or(hint),
            exit_code,
        })
    }
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
