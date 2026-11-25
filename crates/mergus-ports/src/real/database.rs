//! In-process DuckDB with the `ducklake` extension pinned and cached (MG-REQ-0194).

use std::path::{Path, PathBuf};

use arrow::record_batch::RecordBatch;
use duckdb::{Config, Connection};

use crate::{Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, PortError, SqlValue};

#[cfg(test)]
mod tests;

/// The one extension this port installs and loads.
pub(super) const EXTENSION: &str = "ducklake";

/// The host DuckDB downloads its extensions from (MG-REQ-0194.1).
const HOST: &str = "extensions.duckdb.org";

/// DuckDB, unable to install or load any extension on its own and refusing unsigned ones. It
/// installs only `ducklake`, from `extensions.duckdb.org` at the version of the bundled
/// DuckDB, into `mergus/duckdb/extensions/` under the user cache directory, the first time a
/// DuckLake catalog is attached; later uses load it from there.
#[derive(Clone, Debug)]
pub struct RealDatabase {
    extension_dir: PathBuf,
    /// The repository extensions are installed from; DuckDB's own when `None`.
    repository: Option<String>,
}

impl RealDatabase {
    /// Returns a database that caches the `ducklake` extension under `user_cache_dir`, the
    /// user cache directory given by [`crate::Environment::cache_dir`].
    #[must_use]
    pub fn new(user_cache_dir: &Path) -> Self {
        Self {
            extension_dir: user_cache_dir
                .join("mergus")
                .join("duckdb")
                .join("extensions"),
            repository: None,
        }
    }

    /// Returns the directory the `ducklake` extension is cached in.
    #[must_use]
    pub fn extension_dir(&self) -> &Path {
        &self.extension_dir
    }

    /// Returns the host extensions are downloaded from.
    #[must_use]
    pub fn host(&self) -> &str {
        self.repository.as_deref().map_or(HOST, |repository| {
            let rest = repository
                .split_once("://")
                .map_or(repository, |(_, rest)| rest);
            rest.split('/').next().unwrap_or(rest)
        })
    }

    /// Returns the configuration of every connection.
    fn config(&self) -> Result<Config, PortError> {
        let dir = self.extension_dir.to_str().ok_or_else(|| {
            PortError::Failed(format!(
                "the extension directory `{}` is not valid Unicode",
                self.extension_dir.display()
            ))
        })?;
        let mut config = Config::default()
            .enable_autoload_extension(false)
            .and_then(|config| config.with("autoinstall_known_extensions", "false"))
            .and_then(|config| config.with("extension_directory", dir))
            .map_err(failed)?;
        if let Some(repository) = &self.repository {
            config = config
                .with("custom_extension_repository", repository)
                .map_err(failed)?;
        }
        Ok(config)
    }
}

impl Database for RealDatabase {
    fn connect(
        &self,
        location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        let config = self.config()?;
        let connection = match location {
            DatabaseLocation::InMemory => Connection::open_in_memory_with_flags(config),
            DatabaseLocation::File(path) => Connection::open_with_flags(path, config),
        }
        .map_err(failed)?;
        Ok(Box::new(RealConnection {
            connection,
            extension_dir: self.extension_dir.clone(),
            host: self.host().to_owned(),
        }))
    }
}

/// A connection of [`RealDatabase`].
struct RealConnection {
    connection: Connection,
    extension_dir: PathBuf,
    host: String,
}

impl RealConnection {
    /// Installs `ducklake` when it is not cached, then loads it from the cache.
    fn load_ducklake(&self) -> Result<(), PortError> {
        let installed = self
            .connection
            .query_row(
                "SELECT installed FROM duckdb_extensions() WHERE extension_name = ?",
                [EXTENSION],
                |row| row.get::<_, bool>(0),
            )
            .map_err(failed)?;
        if !installed {
            self.connection
                .execute_batch("INSTALL ducklake")
                .map_err(|_| PortError::ExtensionUnavailable {
                    extension: EXTENSION.to_owned(),
                    host: self.host.clone(),
                    cache_dir: self.extension_dir.clone(),
                })?;
        }
        self.connection
            .execute_batch("LOAD ducklake")
            .map_err(failed)
    }
}

impl DatabaseConnection for RealConnection {
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError> {
        self.connection
            .execute(sql, duckdb::params_from_iter(params.iter().map(value)))
            .map_err(failed)
    }

    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError> {
        let mut statement = self.connection.prepare(sql).map_err(failed)?;
        let batches = statement
            .query_arrow(duckdb::params_from_iter(params.iter().map(value)))
            .map_err(failed)?
            .collect();
        Ok(batches)
    }

    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError> {
        let mut appender = self.connection.appender(table).map_err(failed)?;
        appender
            .append_record_batch(batch.clone())
            .and_then(|()| appender.flush())
            .map_err(failed)
    }

    fn attach_ducklake(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.load_ducklake()?;
        let metadata = format!("ducklake:{}", catalog.metadata);
        // sql: identifiers quoted by quote_identifier
        let attach = format!(
            "ATTACH ? AS {} (DATA_PATH ?)",
            quote_identifier(&catalog.alias)
        );
        self.connection
            .execute(&attach, [metadata.as_str(), catalog.data_path.as_str()])
            .map(|_| ())
            .map_err(failed)
    }
}

/// Quotes `identifier` for DuckDB: in double quotes, with each double quote doubled.
fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Converts a bound value into DuckDB's.
fn value(value: &SqlValue) -> duckdb::types::Value {
    match value {
        SqlValue::Null => duckdb::types::Value::Null,
        SqlValue::Boolean(value) => duckdb::types::Value::Boolean(*value),
        SqlValue::Integer(value) => duckdb::types::Value::BigInt(*value),
        SqlValue::Double(value) => duckdb::types::Value::Double(*value),
        SqlValue::Text(value) => duckdb::types::Value::Text(value.clone()),
    }
}

/// Converts a DuckDB error into a [`PortError`].
fn failed(error: duckdb::Error) -> PortError {
    PortError::Failed(format!("DuckDB failed: {error}"))
}
