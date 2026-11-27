//! In-process DuckDB with the `ducklake`, `postgres` and `httpfs` extensions pinned and cached
//! (MG-REQ-0194), which attaches database sources read-only (MG-REQ-0196).

use std::path::{Path, PathBuf};

use arrow::record_batch::RecordBatch;
use duckdb::{Config, Connection};

use crate::database::quote_identifier;
use crate::{
    Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Extension, LoadedExtension,
    PortError, SqlValue,
};

#[cfg(test)]
mod tests;

/// The host DuckDB downloads its extensions from (MG-REQ-0194.1).
const HOST: &str = "extensions.duckdb.org";

/// DuckDB, unable to install or load any extension on its own and refusing unsigned ones. It
/// installs only the extensions of [`Extension`], from `extensions.duckdb.org` at the version
/// of the bundled DuckDB, into `mergus/duckdb/extensions/` under the user cache directory,
/// the first time one is needed; later uses load it from there.
#[derive(Clone, Debug)]
pub struct RealDatabase {
    extension_dir: PathBuf,
    /// The repository extensions are installed from; DuckDB's own when `None`.
    repository: Option<String>,
}

impl RealDatabase {
    /// Returns a database that caches its extensions under `user_cache_dir`, the
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

    /// Returns the directory the extensions are cached in.
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
            loaded: Vec::new(),
        }))
    }
}

/// A connection of [`RealDatabase`].
struct RealConnection {
    connection: Connection,
    extension_dir: PathBuf,
    host: String,
    /// The load registry: every extension loaded, with the version DuckDB reports.
    loaded: Vec<LoadedExtension>,
}

impl RealConnection {
    /// Returns whether `extension` is installed and the version DuckDB reports for it.
    fn status(&self, extension: Extension) -> Result<(bool, String), PortError> {
        let name = extension.name();
        self.connection
            .query_row(
                "SELECT installed, coalesce(extension_version, '') FROM duckdb_extensions() \
                 WHERE extension_name = ? OR list_contains(aliases, ?)",
                [name, name],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, String>(1)?)),
            )
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

    /// Runs the statement with DuckDB's streaming execution (`stream_arrow`), so that DuckDB
    /// produces each batch only when the one before it has been handled, and a fetch error is
    /// returned, never a panic.
    fn query_arrow_each(
        &mut self,
        sql: &str,
        params: &[SqlValue],
        each: &mut dyn FnMut(RecordBatch) -> Result<(), PortError>,
    ) -> Result<(), PortError> {
        let mut statement = self.connection.prepare(sql).map_err(failed)?;
        let batches = statement
            .stream_arrow(duckdb::params_from_iter(params.iter().map(value)))
            .map_err(failed)?;
        for batch in batches {
            each(batch.map_err(failed)?)?;
        }
        Ok(())
    }

    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError> {
        let mut appender = self.connection.appender(table).map_err(failed)?;
        appender
            .append_record_batch(batch.clone())
            .and_then(|()| appender.flush())
            .map_err(failed)
    }

    fn load_extension(&mut self, extension: Extension) -> Result<(), PortError> {
        let name = extension.name();
        let (installed, _) = self.status(extension)?;
        if !installed {
            // The names are those of `Extension`, never user text.
            self.connection
                .execute_batch(&format!("INSTALL {name}"))
                .map_err(|_| PortError::ExtensionUnavailable {
                    extension: name.to_owned(),
                    host: self.host.clone(),
                    cache_dir: self.extension_dir.clone(),
                })?;
        }
        self.connection
            .execute_batch(&format!("LOAD {name}"))
            .map_err(failed)?;
        let (_, version) = self.status(extension)?;
        self.loaded.retain(|loaded| loaded.extension != extension);
        self.loaded.push(LoadedExtension { extension, version });
        Ok(())
    }

    fn loaded_extensions(&self) -> Vec<LoadedExtension> {
        self.loaded.clone()
    }

    fn attach_ducklake(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.load_extension(Extension::DuckLake)?;
        let metadata = format!("ducklake:{}", catalog.metadata);
        let alias = quote_identifier(&catalog.alias);
        // sql: identifiers quoted by quote_identifier
        let attach = format!("ATTACH ? AS {alias} (DATA_PATH ?)");
        self.connection
            .execute(&attach, [metadata.as_str(), catalog.data_path.as_str()])
            .map(|_| ())
            .map_err(failed)
    }

    fn attach_ducklake_encrypted(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.load_extension(Extension::Httpfs)?;
        self.load_extension(Extension::DuckLake)?;
        let metadata = format!("ducklake:{}", catalog.metadata);
        let alias = quote_identifier(&catalog.alias);
        // Data inlining stays off, so that no row is kept in the catalog instead of in an
        // encrypted data file.
        let options = "DATA_PATH ?, ENCRYPTED, DATA_INLINING_ROW_LIMIT 0";
        // sql: identifiers quoted by quote_identifier
        let attach = format!("ATTACH ? AS {alias} ({options})");
        self.connection
            .execute(&attach, [metadata.as_str(), catalog.data_path.as_str()])
            .map(|_| ())
            .map_err(|error| {
                if error.to_string().contains(NOT_ENCRYPTED) {
                    PortError::UnencryptedCatalog {
                        catalog: catalog.metadata.clone(),
                    }
                } else {
                    failed(error)
                }
            })
    }
}

/// The words of DuckLake's refusal to attach an unencrypted catalog with `ENCRYPTED`.
const NOT_ENCRYPTED: &str = "the database is not encrypted but we requested an encrypted database";

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
