//! The failures of a Polars step.

use mergus_ports::PortError;

/// Why a Polars step did not plan or run to its end.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PolarsError {
    /// The step raised an exception, or returned something that is not a LazyFrame
    /// (MG-REQ-0617.4).
    #[error("model `{model}`, step `{step}`: {failure}")]
    StepFailed {
        /// The model.
        model: String,
        /// The step: the function of the model's file.
        step: String,
        /// What went wrong.
        failure: StepFailure,
    },
    /// The Python child cannot import `polars` (MG-REQ-0617.4).
    #[error(
        "model `{model}`, step `{step}`: the project's Python cannot import `polars` ({message}); \
         install polars in the project's Python"
    )]
    PolarsMissing {
        /// The model.
        model: String,
        /// The step.
        step: String,
        /// The message of the `ImportError`.
        message: String,
    },
    /// The Python child stopped without saying how its step went, for example because the
    /// interpreter crashed.
    #[error("model `{model}`, step `{step}`: the Python child failed: {reason}")]
    Child {
        /// The model.
        model: String,
        /// The step.
        step: String,
        /// Its exit code and what it wrote to standard error.
        reason: String,
    },
    /// A file of the step's working directory cannot be encoded or decoded as Arrow IPC.
    #[error("`{path}` is not a readable Arrow IPC file: {reason}")]
    Boundary {
        /// The file, in the step's working directory.
        path: String,
        /// What went wrong.
        reason: String,
    },
    /// The interpreter or the file system failed.
    #[error(transparent)]
    Port(#[from] PortError),
}

/// How a step failed (MG-REQ-0617.4).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StepFailure {
    /// It raised an exception, of the type `kind` with the message `message`.
    #[error("{kind}: {message}")]
    Exception {
        /// The exception's type.
        kind: String,
        /// The exception's message.
        message: String,
    },
    /// It returned a value of the type `returned`, not a LazyFrame.
    #[error("it returned a `{returned}`, not a LazyFrame")]
    NotLazyFrame {
        /// The type of the returned value.
        returned: String,
    },
}

impl PolarsError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008); `None` for a port failure,
    /// which the caller reports with the code of its port error, for a child that stopped
    /// without a report, and for a file the run itself wrote or read.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::StepFailed { .. } => Some("MG-0611"),
            Self::PolarsMissing { .. } => Some("MG-0612"),
            Self::Child { .. } | Self::Boundary { .. } | Self::Port(_) => None,
        }
    }
}
