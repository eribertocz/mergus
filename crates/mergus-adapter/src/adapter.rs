//! The `StorageAdapter` trait.

use arrow::array::RecordBatchReader;

use crate::{AdapterError, Capabilities, PinRecipe, Retention, VersionRef, WriteMode};

/// A stream of Arrow record batches of one table, with its schema.
pub type BatchStream = Box<dyn RecordBatchReader + Send>;

/// The contract every storage adapter implements: exactly these six operations
/// (MG-REQ-0285).
pub trait StorageAdapter: Send {
    /// Declares what the adapter implements, and nothing more (MG-REQ-0287).
    fn capabilities(&self) -> Capabilities;

    /// Writes the Arrow stream `data` to `table` with the write `mode`. Nothing is visible
    /// before [`commit`](Self::commit).
    ///
    /// # Errors
    ///
    /// Returns an [`AdapterError`] when the stream or the storage fails.
    fn write(
        &mut self,
        table: &str,
        mode: WriteMode,
        data: BatchStream,
    ) -> Result<(), AdapterError>;

    /// Commits everything written and returns the reference of the new data version.
    ///
    /// # Errors
    ///
    /// Returns an [`AdapterError`] when the storage cannot commit.
    fn commit(&mut self) -> Result<VersionRef, AdapterError>;

    /// Describes how to read `table` pinned at `version`. The recipe is stored in the
    /// manifest: the canonical JSON of exactly `format`, `table` and `version`, never a
    /// location, a credential or a secret value (MG-REQ-0298).
    ///
    /// # Errors
    ///
    /// Returns an [`AdapterError`] when the version is unknown to the storage.
    fn pin_recipe(&self, table: &str, version: &VersionRef) -> Result<PinRecipe, AdapterError>;

    /// Reads `table` as it was at `version`, for checks, summaries and comparisons.
    ///
    /// # Errors
    ///
    /// Returns an [`AdapterError`] when the table or the version cannot be read.
    fn read_at(&self, table: &str, version: &VersionRef) -> Result<BatchStream, AdapterError>;

    /// Deletes old versions, never one in [`Retention::live`] or inside
    /// [`Retention::rollback_window`].
    ///
    /// # Errors
    ///
    /// Returns an [`AdapterError`] when the storage cannot delete.
    fn expire(&mut self, retention: &Retention) -> Result<(), AdapterError>;
}
