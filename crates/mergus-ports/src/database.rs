//! The `Database` port: in-process DuckDB connections that run SQL, read and write Arrow and
//! attach DuckLake catalogs (MG-REQ-0194).

use std::path::PathBuf;

use arrow::record_batch::RecordBatch;

use crate::PortError;

/// A value bound to a parameter of a SQL statement. Values are always bound, never
/// concatenated into the SQL text (MG-REQ-0007).
#[derive(Clone, Debug, PartialEq)]
pub enum SqlValue {
    /// SQL `NULL`.
    Null,
    /// A `BOOLEAN`.
    Boolean(bool),
    /// A `BIGINT`.
    Integer(i64),
    /// A `DOUBLE`.
    Double(f64),
    /// A `VARCHAR`.
    Text(String),
}

/// A DuckLake catalog to attach to a connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuckLakeCatalog {
    /// The name the catalog is attached as.
    pub alias: String,
    /// The metadata database of the catalog, such as a DuckDB file.
    pub metadata: String,
    /// The directory or object-store prefix of the catalog's data files.
    pub data_path: String,
}

/// Where a connection keeps its database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatabaseLocation {
    /// A database in memory, gone when the connection closes.
    InMemory,
    /// The DuckDB file at this path, created when it does not exist.
    File(PathBuf),
}

/// In-process DuckDB.
pub trait Database: Send + Sync {
    /// Opens a connection to the database at `location`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the database cannot be opened.
    fn connect(
        &self,
        location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError>;
}

/// An open DuckDB connection.
pub trait DatabaseConnection: Send {
    /// Runs the statement `sql` with `params` bound to its parameters, and returns the number
    /// of rows it changed.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the statement fails.
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError>;

    /// Runs the query `sql` with `params` bound to its parameters, and returns its rows as
    /// Arrow record batches.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the query fails.
    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError>;

    /// Appends the rows of `batch` to the existing table `table` of the main schema.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the table does not exist or does not match the
    /// batch.
    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError>;

    /// Attaches the DuckLake catalog `catalog`, loading the `ducklake` extension first.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::ExtensionUnavailable`] when the extension is not cached and
    /// cannot be downloaded, and with [`PortError::Failed`] when the catalog cannot be
    /// attached.
    fn attach_ducklake(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError>;
}
