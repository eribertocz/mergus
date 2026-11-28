//! Opening the raw catalog encrypted (MG-REQ-0515).

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use arrow::record_batch::RecordBatch;
use mergus_adapter::AdapterError;
use mergus_adapter_ducklake::{DuckLakeAdapter, DuckLakeError};
use mergus_ports::{
    Clock, Database, DatabaseConnection, DatabaseLocation, DuckLakeAccess, DuckLakeCatalog,
    Extension, LoadedExtension, PortError, ResultColumn, SqlValue,
};

use crate::{Catalog, RawError, ZoneStorage};

/// The name the raw catalog is attached as, so that its tables read `raw.<table>`.
pub const RAW_ALIAS: &str = "raw";

/// Opens the DuckLake adapter of the raw zone `storage`, of the project at `root`, with its
/// catalog attached encrypted (MG-REQ-0515.1). `connection` is the resolved value of the
/// catalog secret of a `cloud` zone; it is ignored for `local` and `shared`.
///
/// # Errors
///
/// - [`RawError::CatalogInsideProject`] when a `local` or `shared` catalog resolves inside
///   `root`; nothing is attached (MG-REQ-0515.2);
/// - [`RawError::UnencryptedCatalog`] when the catalog exists and is not encrypted
///   (MG-REQ-0515.3);
/// - [`RawError::MissingConnection`] for a `cloud` zone without `connection`;
/// - [`RawError::Storage`] when the catalog cannot be attached.
pub fn open_raw(
    database: &dyn Database,
    storage: &ZoneStorage,
    root: &Path,
    connection: Option<&str>,
    clock: Arc<dyn Clock>,
) -> Result<DuckLakeAdapter, RawError> {
    let catalog = raw_catalog(storage, root, connection)?;
    DuckLakeAdapter::open(&Encrypted(database), &catalog, clock).map_err(|error| {
        if is_unencrypted(&error) {
            RawError::UnencryptedCatalog {
                catalog: catalog_name(&storage.catalog),
            }
        } else {
            RawError::Storage(error)
        }
    })
}

/// The DuckLake catalog of the raw zone `storage`, of the project at `root`, attached as
/// [`RAW_ALIAS`]. `connection` is the resolved value of the catalog secret of a `cloud`
/// zone; it is ignored for `local` and `shared`. Nothing is attached.
///
/// # Errors
///
/// - [`RawError::CatalogInsideProject`] when a `local` or `shared` catalog resolves inside
///   `root` (MG-REQ-0515.2);
/// - [`RawError::MissingConnection`] for a `cloud` zone without `connection`.
pub fn raw_catalog(
    storage: &ZoneStorage,
    root: &Path,
    connection: Option<&str>,
) -> Result<DuckLakeCatalog, RawError> {
    if let Catalog::DuckDb { file } | Catalog::Sqlite { file } = &storage.catalog {
        check_outside(file, root, &storage.catalog)?;
    }
    let metadata = storage
        .catalog
        .attach(connection)
        .and_then(|attach| attach.strip_prefix("ducklake:").map(str::to_owned))
        .ok_or_else(|| RawError::MissingConnection {
            secret: catalog_name(&storage.catalog),
        })?;
    Ok(DuckLakeCatalog {
        alias: RAW_ALIAS.to_owned(),
        metadata,
        data_path: storage.data_path.clone(),
    })
}

/// The snapshot of raw every segment of a run reads (MG-REQ-0704): the latest snapshot of
/// `adapter`, taken once after the extractions; `None` when raw has no commit of Mergus yet.
///
/// # Errors
///
/// Fails with [`RawError::Storage`] when the snapshots cannot be read.
pub fn raw_snapshot(adapter: &DuckLakeAdapter) -> Result<Option<i64>, RawError> {
    Ok(adapter.latest_snapshot()?)
}

/// The name errors give `catalog` by: its file, or the reference of its secret, never the
/// secret's value.
fn catalog_name(catalog: &Catalog) -> String {
    match catalog {
        Catalog::DuckDb { file } | Catalog::Sqlite { file } => file.clone(),
        Catalog::Postgres { secret } => secret.clone(),
    }
}

/// Fails when the catalog file `file` resolves inside `root` (MG-REQ-0515.2).
fn check_outside(file: &str, root: &Path, catalog: &Catalog) -> Result<(), RawError> {
    let root = normalize(root);
    if !normalize(&root.join(file)).starts_with(&root) {
        return Ok(());
    }
    let project = root
        .file_name()
        .map_or_else(|| "project".into(), |name| name.to_string_lossy());
    let file_name = match catalog {
        Catalog::Sqlite { .. } => "catalog.sqlite",
        Catalog::DuckDb { .. } | Catalog::Postgres { .. } => "catalog.ducklake",
    };
    let parent = root.parent().unwrap_or(&root);
    let suggestion = parent.join(format!("{project}-raw")).join(file_name);
    Err(RawError::CatalogInsideProject {
        path: file.to_owned(),
        suggestion: suggestion.display().to_string(),
    })
}

/// `path` with its `.` parts dropped and each `..` part removing the part before it.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    normal
}

/// Whether `error` is the port's refusal of an unencrypted catalog.
fn is_unencrypted(error: &AdapterError) -> bool {
    let AdapterError::Storage(inner) = error else {
        return false;
    };
    matches!(
        inner.downcast_ref::<DuckLakeError>(),
        Some(DuckLakeError::Port(PortError::UnencryptedCatalog { .. }))
    )
}

/// A `Database` whose connections attach every DuckLake catalog encrypted.
struct Encrypted<'a>(&'a dyn Database);

impl Database for Encrypted<'_> {
    fn connect(
        &self,
        location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        Ok(Box::new(EncryptedConnection(self.0.connect(location)?)))
    }
}

/// A connection of [`Encrypted`].
struct EncryptedConnection(Box<dyn DatabaseConnection>);

impl DatabaseConnection for EncryptedConnection {
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError> {
        self.0.execute(sql, params)
    }

    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError> {
        self.0.query_arrow(sql, params)
    }

    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError> {
        self.0.append_arrow(table, batch)
    }

    /// Attaches `catalog` encrypted, read-write: raw is never attached otherwise
    /// (MG-REQ-0515.3).
    fn attach_ducklake(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.0
            .attach_ducklake_encrypted(catalog, &DuckLakeAccess::default())
    }

    fn attach_ducklake_encrypted(
        &mut self,
        catalog: &DuckLakeCatalog,
        access: &DuckLakeAccess,
    ) -> Result<(), PortError> {
        self.0.attach_ducklake_encrypted(catalog, access)
    }

    fn load_extension(&mut self, extension: Extension) -> Result<(), PortError> {
        self.0.load_extension(extension)
    }

    fn loaded_extensions(&self) -> Vec<LoadedExtension> {
        self.0.loaded_extensions()
    }

    fn describe(&mut self, sql: &str, params: &[SqlValue]) -> Result<Vec<ResultColumn>, PortError> {
        self.0.describe(sql, params)
    }

    fn create_empty_table(
        &mut self,
        schema: Option<&str>,
        table: &str,
        columns: &[ResultColumn],
    ) -> Result<(), PortError> {
        self.0.create_empty_table(schema, table, columns)
    }
}

#[cfg(test)]
mod tests {
    use mergus_ports::{Database, DatabaseLocation, ResultColumn, SqlValue};
    use mergus_sim::{DatabaseFaults, SimDatabase};

    use super::Encrypted;

    #[test]
    fn encrypted_connection_forwards_describe_and_create_empty_table() {
        let inner = SimDatabase::new(0, DatabaseFaults::default());
        let columns = vec![ResultColumn {
            name: "id".to_owned(),
            data_type: "BIGINT".to_owned(),
        }];
        let params = vec![SqlValue::Date(0)];
        inner.record_describe("SELECT id FROM raw.t", params.clone(), columns.clone());
        let mut connection = Encrypted(&inner)
            .connect(&DatabaseLocation::InMemory)
            .unwrap();
        connection
            .create_empty_table(Some("raw"), "t", &columns)
            .unwrap();
        assert_eq!(
            connection
                .describe("SELECT id FROM raw.t", &params)
                .unwrap(),
            columns
        );
        assert_eq!(
            inner.created(),
            [(Some("raw".to_owned()), "t".to_owned(), columns)]
        );
        assert_eq!(
            inner.described(),
            [("SELECT id FROM raw.t".to_owned(), params)]
        );
        // The default `create_empty_table` would run DDL through `execute` instead.
        assert!(inner.executed().is_empty());
    }
}
