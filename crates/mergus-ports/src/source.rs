//! The `SourceConnector` port: ADBC connections and file readers of sources.

use crate::PortError;

/// Reads the data of one source, batch by batch.
pub trait SourceReader: Send {
    /// Returns the next batch, or `None` when the source has no more data.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the source fails.
    fn next_batch(&mut self) -> Result<Option<Vec<u8>>, PortError>;
}

/// ADBC connections and file readers of sources.
pub trait SourceConnector: Send + Sync {
    /// Opens the source named `source` for reading.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the source does not exist, and with
    /// [`PortError::Failed`] when it cannot be opened.
    fn open(&self, source: &str) -> Result<Box<dyn SourceReader>, PortError>;
}
