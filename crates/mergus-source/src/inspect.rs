//! Why a source could not be inspected: `mergus doctor --source` fails with
//! [`InspectError`] (MG-REQ-0431), and `mergus plan` warns with [`ImpactUnknown`] and shows
//! `unknown` (MG-REQ-0432.2).

/// A source that `mergus doctor --source` could not reach or whose catalog it could not
/// read (MG-REQ-0008).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InspectError {
    /// The source could not be attached: no DuckDB is available, or the connection failed.
    #[error("source `{source_name}` cannot be reached: {detail}")]
    Unreachable {
        /// The source name.
        source_name: String,
        /// Why.
        detail: String,
    },
    /// The source was attached, but its catalog (`pg_index`) could not be read.
    #[error("the catalog of source `{source_name}` cannot be read: {detail}")]
    Unreadable {
        /// The source name.
        source_name: String,
        /// Why.
        detail: String,
    },
}

impl InspectError {
    /// The `MG-####` code of the error (MG-REQ-0008).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "MG-0408"
    }
}

/// A warning of `mergus plan`: what reading a source costs is unknown, because the source
/// could not be reached or read; `plan` still succeeds (MG-REQ-0432.2).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the estimated rows and indexes of source `{source_name}` are unknown: {reason}")]
pub struct ImpactUnknown {
    /// The source name.
    pub source_name: String,
    /// Why it could not be reached or read.
    pub reason: String,
}

impl ImpactUnknown {
    /// The `MG-####` code of the warning (MG-REQ-0432.2).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "MG-0409"
    }
}
