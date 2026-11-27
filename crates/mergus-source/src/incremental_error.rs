//! The failures and warnings of an incremental load (MG-REQ-0415.2, MG-REQ-0420.2).

use mergus_ports::PortError;

/// Why a table could not be read incrementally.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExtractError {
    /// A table declares `deletes = "column:<name>"` and the source has no such column
    /// (MG-REQ-0415.2). Nothing of the table was read.
    #[error(
        "table `{table}` of source `{source_name}` declares `deletes = \"column:{column}\"` but \
         the source table has no column `{column}`"
    )]
    MissingDeleteColumn {
        /// The source.
        source_name: String,
        /// The declared table.
        table: String,
        /// The delete column.
        column: String,
    },
    /// The source or the rows read failed.
    #[error(transparent)]
    Port(#[from] PortError),
}

impl ExtractError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008); `None` for a port failure,
    /// which the caller reports with the code of its port error.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::MissingDeleteColumn { .. } => Some("MG-0410"),
            Self::Port(_) => None,
        }
    }
}

/// Rows that arrived with an incremental value older than `watermark − overlap`, found by a
/// `compare` and not loaded (MG-REQ-0420). A warning: it never changes the exit code.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "table {source_name}.{table}: {count} rows older than {since} arrived late; run mergus \
     backfill --source {source_name} --table {table} --from {oldest}"
)]
pub struct LateRows {
    /// The source.
    pub source_name: String,
    /// The declared table.
    pub table: String,
    /// How many rows arrived late.
    pub count: u64,
    /// `watermark − overlap`, the start of the reads, in DuckDB's text.
    pub since: String,
    /// The oldest incremental value of the late rows, in DuckDB's text.
    pub oldest: String,
}

impl LateRows {
    /// Returns the `MG-####` code of the warning (MG-REQ-0420.2).
    #[must_use]
    pub fn code(&self) -> &'static str {
        "MG-0411"
    }
}
