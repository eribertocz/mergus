//! The failures of a segment.

use mergus_ports::PortError;

/// Why a segment did not run to its end.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EngineError {
    /// A value, or a column, of an input or of the output cannot be represented exactly in
    /// the type its schema declares: a DECIMAL overflow, a HUGEINT beyond BIGINT, a lost
    /// digit, a changed time zone, a Parquet column of another type, or an Arrow type DuckDB
    /// has no exact type for (MG-REQ-0180). Nothing was rounded, truncated or cast, and no
    /// output was written.
    #[error("table `{table}`, column `{column}`: {reason}")]
    Unrepresentable {
        /// The input, as the segment's SQL names it, or the output.
        table: String,
        /// The column.
        column: String,
        /// What cannot be represented, and in which type.
        reason: String,
    },
    /// The DuckDB session has the `icu` extension loaded. With it, DuckDB gives a
    /// `TIMESTAMPTZ` back in the session's time zone, which can be the host's, so zoned
    /// timestamps would depend on the machine (MG-REQ-0180). The step failed before it ran.
    #[error(
        "the DuckDB session has the `icu` extension loaded, so time zones would depend on the host"
    )]
    ZoneExtensionLoaded,
    /// A boundary file cannot be encoded or decoded as Arrow IPC.
    #[error("the boundary `{path}` is not a readable Arrow IPC file: {reason}")]
    Boundary {
        /// The file, in the run's work directory.
        path: String,
        /// What went wrong.
        reason: String,
    },
    /// A scratch database of a Parquet output, its write-ahead log or its spill directory
    /// exists and cannot be removed, for example because another process holds it.
    #[error("the scratch database `{path}` cannot be removed: {reason}")]
    Scratch {
        /// The file or directory.
        path: String,
        /// What went wrong.
        reason: String,
    },
    /// A value of a table cannot be displayed for its hash (MG-REQ-0638).
    #[error("a value cannot be displayed for the table hash: {reason}")]
    Hash {
        /// What went wrong.
        reason: String,
    },
    /// DuckDB or the file system failed.
    #[error(transparent)]
    Port(#[from] PortError),
}

impl EngineError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008); `None` for a port failure,
    /// which the caller reports with the code of its port error, and for a boundary the run
    /// itself wrote or read.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Unrepresentable { .. } => Some("MG-0101"),
            Self::ZoneExtensionLoaded => Some("MG-0102"),
            Self::Boundary { .. } | Self::Scratch { .. } | Self::Hash { .. } | Self::Port(_) => {
                None
            }
        }
    }
}
