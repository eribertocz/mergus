//! Why a run cycle failed.

use std::path::Path;

use mergus_ports::PortError;

/// A failure of one extraction or one step, with the code of the crate that found it.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{name}: {message}")]
pub struct StepFailure {
    /// The table extracted or the model computed.
    pub name: String,
    /// Its `MG-####` code (MG-REQ-0008); `None` for a port failure without one.
    pub code: Option<&'static str>,
    /// What failed.
    pub message: String,
}

/// Why stages 1 to 5 of a run, or the cleanup of its work zone, failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// The scratch directory or a candidate could not be read, written or removed
    /// (MG-REQ-0503, MG-REQ-0705).
    #[error("the work zone cannot be used at `{path}`: {message}")]
    Work {
        /// The path, relative to the project root when it lies inside it.
        path: String,
        /// What the file system reported.
        message: String,
    },
    /// An extraction failed, so no segment ran (MG-REQ-0704).
    #[error("extraction failed: {0}")]
    Extraction(StepFailure),
    /// A step failed; steps already running were let finish and no other one started.
    #[error("step failed: {0}")]
    Step(StepFailure),
    /// The plan computes a model the caller gave no work for.
    #[error("internal error: the plan computes `{0}`, but no work was given for it")]
    NoWork(String),
    /// A computed step reads a step of the plan that never finished, though none failed.
    #[error("internal error: the step of `{0}` reads a step that never finished")]
    Unordered(String),
}

impl RunError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008); `None` for an internal error
    /// and for a failure whose crate gave none.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Work { .. } => Some("MG-0704"),
            Self::Extraction(failure) | Self::Step(failure) => failure.code,
            Self::NoWork(_) | Self::Unordered(_) => None,
        }
    }

    /// The [`RunError::Work`] of `error` at `path`, shown relative to `root`.
    pub(crate) fn work(root: &Path, path: &Path, error: &PortError) -> Self {
        let shown = path.strip_prefix(root).unwrap_or(path);
        Self::Work {
            path: shown.display().to_string().replace('\\', "/"),
            message: error.to_string(),
        }
    }
}
