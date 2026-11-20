//! The error every command returns.

use std::fmt;
use std::io;
use std::path::Path;

/// A check that did not pass, or a step that could not run.
///
/// `main` returns it, so the standard library prints it on standard error through its
/// `Debug` implementation and the process exits with a non-zero status. `Debug` therefore
/// prints the plain message.
pub(crate) struct Failure {
    message: String,
}

impl Failure {
    /// Creates a failure with a message for the developer.
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Creates a failure for an I/O error on `path`.
    pub(crate) fn io(path: &Path, error: &io::Error) -> Self {
        Self::new(format!("{}: {error}", path.display()))
    }

    /// Creates a failure from a list of problems, or returns `Ok` when the list is empty.
    ///
    /// # Errors
    ///
    /// Returns a failure that names the check and lists every problem, one per line.
    pub(crate) fn from_problems(check: &str, problems: &[String]) -> Result<(), Self> {
        if problems.is_empty() {
            return Ok(());
        }
        Err(Self::new(format!(
            "check `{check}` failed:\n  {}",
            problems.join("\n  ")
        )))
    }
}

impl fmt::Debug for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::Failure;

    #[test]
    fn no_problem_is_success() {
        assert!(Failure::from_problems("clippy", &[]).is_ok());
    }

    #[test]
    fn problems_are_listed_one_per_line() {
        let problems = ["a".to_owned(), "b".to_owned()];
        let failure = Failure::from_problems("unsafe", &problems).unwrap_err();
        assert_eq!(format!("{failure:?}"), "check `unsafe` failed:\n  a\n  b");
    }
}
