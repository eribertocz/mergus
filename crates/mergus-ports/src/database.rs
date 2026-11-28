//! The `Database` port: in-process DuckDB connections that run SQL, read and write Arrow,
//! attach DuckLake catalogs (MG-REQ-0194) and attach database sources read-only
//! (MG-REQ-0196).

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use arrow::record_batch::RecordBatch;

use crate::PortError;
use crate::column::{ResultColumn, is_column_type};
use crate::ducklake::{DuckLakeAccess, DuckLakeCatalog};
use crate::macros::MacroRun;
use crate::session::protected_connection;

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
    /// A `DATE`: days since 1970-01-01.
    Date(i32),
    /// A `TIMESTAMPTZ`: microseconds since 1970-01-01 00:00:00 UTC. The `duckdb` crate binds
    /// no `TIMESTAMPTZ`, so it is bound as the `TIMESTAMP` of that UTC instant, which the SQL
    /// reads with `CAST(<parameter> AS TIMESTAMPTZ)`. Without the `icu` extension, which
    /// MG-REQ-0194 does not load, DuckDB casts it as UTC: the instant is kept.
    TimestampTz(i64),
}

/// A DuckDB extension the port may install and load (MG-REQ-0194).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Extension {
    /// `ducklake`, for DuckLake catalogs.
    DuckLake,
    /// `postgres`, for PostgreSQL sources (MG-REQ-0196.2).
    Postgres,
    /// `httpfs`, whose crypto module DuckDB needs to write the files of an encrypted DuckLake
    /// catalog (MG-REQ-0515).
    Httpfs,
}

impl Extension {
    /// Returns the name DuckDB installs and loads the extension by.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DuckLake => "ducklake",
            Self::Postgres => "postgres",
            Self::Httpfs => "httpfs",
        }
    }
}

/// An extension a connection loaded, with the version DuckDB reports for it
/// (MG-REQ-0194.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedExtension {
    /// The extension.
    pub extension: Extension,
    /// The version DuckDB reports for it once loaded.
    pub version: String,
}

/// A PostgreSQL source to attach read-only (MG-REQ-0196.2 and MG-REQ-0196.3).
#[derive(Clone, PartialEq, Eq)]
pub struct PostgresSource {
    /// The name the source is attached as.
    pub alias: String,
    /// The resolved connection value of the source's `connection` secret (MG-REQ-1001), a
    /// libpq connection string or URI. It is never shown by `Debug`.
    pub connection: String,
    /// The source's `max_connections`.
    pub max_connections: u32,
    /// The source's `query_timeout`.
    pub query_timeout: Duration,
}

impl fmt::Debug for PostgresSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresSource")
            .field("alias", &self.alias)
            .field("connection", &"<redacted>")
            .field("max_connections", &self.max_connections)
            .field("query_timeout", &self.query_timeout)
            .finish()
    }
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

    /// Runs `script`, the text of one macro file, in a fresh in-memory session opened with
    /// external access off and the configuration locked, so that no file, network, `ATTACH`,
    /// `INSTALL`, `LOAD` or `SET` can succeed (MG-REQ-0641.1); and returns what the session
    /// shows before and after it ran (MG-REQ-0641.2, MG-REQ-0641.3, MG-REQ-0642.2). A refused
    /// statement is part of the answer, not a failure.
    ///
    /// The default fails: only a database that can open such a session implements it.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the session cannot be opened or its catalog
    /// cannot be read.
    fn run_macros(&self, script: &str) -> Result<MacroRun, PortError> {
        let _ = script;
        Err(PortError::Failed(
            "this database cannot run macro files".to_owned(),
        ))
    }
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

    /// Runs the query `sql` with `params` bound to its parameters, and calls `each` with each
    /// Arrow record batch of its rows, in order, as the database produces them, so that the
    /// caller never holds more than one batch. It stops at the first error of `each`.
    ///
    /// The default collects the rows with [`DatabaseConnection::query_arrow`] first; the real
    /// and simulated databases stream them. A connection that wraps another must forward this
    /// method, or it collects.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the query fails, and with the error of `each`
    /// when it fails.
    fn query_arrow_each(
        &mut self,
        sql: &str,
        params: &[SqlValue],
        each: &mut dyn FnMut(RecordBatch) -> Result<(), PortError>,
    ) -> Result<(), PortError> {
        for batch in self.query_arrow(sql, params)? {
            each(batch)?;
        }
        Ok(())
    }

    /// Returns the columns of the result of the query `sql` with `params` bound to its
    /// parameters, as DuckDB binds it, without running it, so no data is read
    /// (MG-REQ-0604).
    ///
    /// A connection that wraps another MUST forward this method: the default fails.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Refused`] when DuckDB refuses the query (a parser, catalog or
    /// binder error), and with [`PortError::Failed`] when DuckDB fails otherwise or the
    /// connection cannot describe queries.
    fn describe(&mut self, sql: &str, params: &[SqlValue]) -> Result<Vec<ResultColumn>, PortError> {
        let _ = params;
        Err(PortError::Failed(format!(
            "this connection cannot describe `{sql}`"
        )))
    }

    /// Creates the empty table `table`, in the schema `schema` (created when missing) or in
    /// the main schema, with `columns`: their names quoted, their DuckDB types as written
    /// when each is a scalar type of [`is_column_type`].
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::UnsupportedType`] for a type that is not, before any
    /// statement, and with [`PortError::Failed`] when a statement fails.
    fn create_empty_table(
        &mut self,
        schema: Option<&str>,
        table: &str,
        columns: &[ResultColumn],
    ) -> Result<(), PortError> {
        if let Some(column) = columns
            .iter()
            .find(|column| !is_column_type(&column.data_type))
        {
            return Err(PortError::UnsupportedType(column.data_type.clone()));
        }
        let mut name = quote_identifier(table);
        if let Some(schema) = schema {
            let schema = quote_identifier(schema);
            // sql: identifiers quoted by quote_identifier
            self.execute(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"), &[])?;
            name = format!("{schema}.{name}");
        }
        // Every type is one of `is_column_type`, never other text.
        let columns: Vec<String> = columns
            .iter()
            .map(|column| format!("{} {}", quote_identifier(&column.name), column.data_type))
            .collect();
        let columns = columns.join(", ");
        // sql: identifiers quoted by quote_identifier
        self.execute(&format!("CREATE TABLE {name} ({columns})"), &[])
            .map(|_| ())
    }

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

    /// Attaches the DuckLake catalog `catalog` with the `ENCRYPTED` option, so that DuckLake
    /// encrypts every data and delete file it writes with a key of its own, kept in the
    /// catalog (MG-REQ-0515.1). It loads `httpfs`, whose crypto module DuckDB needs to write
    /// encrypted files, and `ducklake` first. A new catalog is created encrypted. `access`
    /// attaches it `READ_ONLY`, at a `SNAPSHOT_VERSION`, or both.
    ///
    /// The default fails: a connection that does not implement it never writes encrypted.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::UnencryptedCatalog`] when `catalog` exists and is not
    /// encrypted (MG-REQ-0515.3), with [`PortError::ExtensionUnavailable`] when an extension
    /// is not cached and cannot be downloaded, and with [`PortError::Failed`] when the
    /// catalog cannot be attached.
    fn attach_ducklake_encrypted(
        &mut self,
        catalog: &DuckLakeCatalog,
        _access: &DuckLakeAccess,
    ) -> Result<(), PortError> {
        Err(PortError::Failed(format!(
            "this connection cannot attach the DuckLake catalog `{}` encrypted",
            catalog.metadata
        )))
    }

    /// Loads `extension`, installing it first when it is not cached, and records the version
    /// DuckDB reports for it in [`DatabaseConnection::loaded_extensions`].
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::ExtensionUnavailable`] when the extension is not cached and
    /// cannot be downloaded, and with [`PortError::Failed`] when it cannot be loaded.
    fn load_extension(&mut self, extension: Extension) -> Result<(), PortError>;

    /// Returns the extensions this connection loaded, with their versions, in load order.
    fn loaded_extensions(&self) -> Vec<LoadedExtension>;

    /// Attaches the DuckDB file at `location` read-only, with no extension, as
    /// `ATTACH '<location>' (READ_ONLY)` (MG-REQ-0196.1). DuckDB names it after the file
    /// name without its extension.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the path is not valid Unicode or the file cannot
    /// be attached.
    fn attach_duckdb_file(&mut self, location: &Path) -> Result<(), PortError> {
        let location = location.to_str().ok_or_else(|| {
            PortError::Failed(format!(
                "the DuckDB file `{}` is not valid Unicode",
                location.display()
            ))
        })?;
        let params = [SqlValue::Text(location.to_owned())];
        self.execute("ATTACH ? (READ_ONLY)", &params).map(|_| ())
    }

    /// Attaches the PostgreSQL source `source` read-only with the `postgres` extension
    /// (MG-REQ-0196.2): sets the pool to `max_connections` connections, acquired in `wait`
    /// mode, and the statement timeout to `query_timeout` (MG-REQ-0196.3); puts the
    /// connection value, with the session parameters of MG-REQ-0429.2 added, into a
    /// temporary secret; then attaches with
    /// `ATTACH '' AS <alias> (TYPE postgres, SECRET <secret>, READ_ONLY)`. The secret is
    /// dropped when the attach fails.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::SessionParameterSet`] when the connection value already sets
    /// `application_name` or `options` (MG-REQ-0429.3), before any statement; with
    /// [`PortError::ExtensionUnavailable`] when the extension is not cached and cannot be
    /// downloaded; and with [`PortError::Failed`] when a statement fails.
    fn attach_postgres(&mut self, source: &PostgresSource) -> Result<(), PortError> {
        let connection = protected_connection(&source.alias, &source.connection)?;
        self.load_extension(Extension::Postgres)?;
        let timeout = i64::try_from(source.query_timeout.as_millis()).unwrap_or(i64::MAX);
        let settings = [
            (
                "SET pg_pool_max_connections = ?",
                SqlValue::Integer(i64::from(source.max_connections)),
            ),
            (
                "SET pg_pool_acquire_mode = ?",
                SqlValue::Text("wait".to_owned()),
            ),
            (
                "SET pg_statement_timeout_millis = ?",
                SqlValue::Integer(timeout),
            ),
        ];
        for (sql, value) in settings {
            self.execute(sql, &[value])?;
        }
        let alias = quote_identifier(&source.alias);
        let secret = quote_identifier(&secret_name(&source.alias));
        // sql: identifiers quoted by quote_identifier
        let create = format!("CREATE TEMPORARY SECRET {secret} (TYPE postgres, URI ?)");
        self.execute(&create, &[SqlValue::Text(connection)])?;
        // sql: identifiers quoted by quote_identifier
        let attach = format!("ATTACH '' AS {alias} (TYPE postgres, SECRET {secret}, READ_ONLY)");
        if let Err(error) = self.execute(&attach, &[]) {
            // The attach error is the one reported; the secret is dropped on a best effort.
            self.execute(&drop_secret(&source.alias), &[]).ok();
            return Err(error);
        }
        Ok(())
    }

    /// Detaches the source attached as `alias` and drops its secret, if it has one
    /// (MG-REQ-0196.2).
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when nothing is attached as `alias` or the secret
    /// cannot be dropped.
    fn detach(&mut self, alias: &str) -> Result<(), PortError> {
        let quoted = quote_identifier(alias);
        self.execute(&format!("DETACH {quoted}"), &[])?;
        self.execute(&drop_secret(alias), &[]).map(|_| ())
    }
}

/// Returns the name of the temporary secret of the source attached as `alias`.
fn secret_name(alias: &str) -> String {
    format!("mergus_source_{alias}")
}

/// Returns the statement that drops the secret of the source attached as `alias`.
fn drop_secret(alias: &str) -> String {
    let secret = quote_identifier(&secret_name(alias));
    // sql: identifiers quoted by quote_identifier
    format!("DROP TEMPORARY SECRET IF EXISTS {secret}")
}

/// Quotes `identifier` for DuckDB: in double quotes, with each double quote doubled.
pub(crate) fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
