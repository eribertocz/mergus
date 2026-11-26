//! The `DuckLakeAdapter`.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arrow::array::{RecordBatch, RecordBatchIterator};
use arrow::datatypes::Schema;
use mergus_adapter::{
    AdapterError, BatchStream, Capabilities, PinRecipe, Retention, StorageAdapter, VersionRef,
    Versioning, WriteMode,
};
use mergus_contracts::canonical_json;
use mergus_ports::{
    Clock, Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, SqlValue,
};
use serde_json::json;

use crate::error::port;
use crate::snapshots::{self, snapshot_id};
use crate::{DuckLakeError, sql};

/// The write modes the adapter implements.
const WRITE_MODES: [WriteMode; 2] = [WriteMode::Append, WriteMode::Replace];

/// Rows written to one table and not committed yet.
#[derive(Debug)]
struct Staged {
    table: String,
    mode: WriteMode,
    stage: String,
}

/// A storage adapter over one DuckLake catalog, reached through the `Database` port.
///
/// `write` stages rows in tables of the connection's in-memory database, outside the
/// catalog, so readers of the published version never see them (MG-REQ-0286). `commit`
/// moves every staged table into the catalog in one transaction, which DuckLake publishes as
/// one snapshot; the snapshot id is the [`VersionRef`].
pub struct DuckLakeAdapter {
    connection: Mutex<Box<dyn DatabaseConnection>>,
    alias: String,
    clock: Arc<dyn Clock>,
    staged: Vec<Staged>,
    next_stage: u64,
}

impl DuckLakeAdapter {
    /// Opens an in-memory connection of `database` and attaches `catalog` to it. `clock`
    /// tells which snapshots are inside the rollback window.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Storage`] when the connection cannot be opened or the catalog
    /// cannot be attached.
    pub fn open(
        database: &dyn Database,
        catalog: &DuckLakeCatalog,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, AdapterError> {
        let mut connection = database
            .connect(&DatabaseLocation::InMemory)
            .map_err(port)?;
        connection.attach_ducklake(catalog).map_err(port)?;
        Ok(Self {
            connection: Mutex::new(connection),
            alias: catalog.alias.clone(),
            clock,
            staged: Vec::new(),
            next_stage: 0,
        })
    }

    fn connection(&self) -> MutexGuard<'_, Box<dyn DatabaseConnection>> {
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn alias(&self) -> SqlValue {
        SqlValue::Text(self.alias.clone())
    }

    /// Runs the query `sql` with `params`.
    fn query(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<RecordBatch>, AdapterError> {
        self.connection().query_arrow(sql, params).map_err(port)
    }

    /// Runs the statement `sql` without parameters.
    fn execute(&self, sql: &str) -> Result<(), AdapterError> {
        self.connection()
            .execute(sql, &[])
            .map(|_| ())
            .map_err(port)
    }

    /// Returns whether `version` names a snapshot of the catalog.
    fn exists(&self, version: &VersionRef) -> Result<bool, AdapterError> {
        let Some(id) = snapshot_id(version) else {
            return Ok(false);
        };
        let rows = self.query(sql::SNAPSHOT_EXISTS, &[self.alias(), SqlValue::Integer(id)])?;
        let found = snapshots::single_int(&rows, sql::SNAPSHOT_EXISTS)?;
        Ok(found.unwrap_or(0) > 0)
    }

    /// Returns the newest snapshot of the catalog.
    fn latest(&self) -> Result<VersionRef, AdapterError> {
        let rows = self.query(sql::LATEST, &[self.alias()])?;
        let latest = snapshots::single_int(&rows, sql::LATEST)?
            .ok_or_else(|| DuckLakeError::UnexpectedRows(sql::LATEST.to_owned()))?;
        Ok(VersionRef::new(latest.to_string()))
    }

    /// Creates the staging table `stage` with `schema` and appends `data` to it.
    fn stage(&self, stage: &str, schema: &Schema, data: BatchStream) -> Result<(), AdapterError> {
        let create = sql::create_stage(stage, schema)?;
        self.execute(&create)?;
        for batch in data {
            self.connection()
                .append_arrow(stage, &batch?)
                .map_err(port)?;
        }
        Ok(())
    }

    /// Moves every staged table into the catalog, inside the open transaction.
    fn publish(&self) -> Result<(), AdapterError> {
        for staged in &self.staged {
            let target = sql::qualified(&self.alias, &staged.table);
            let stage = sql::quote(&staged.stage);
            self.execute(&sql::create_target(&target, &stage))?;
            if staged.mode == WriteMode::Replace {
                self.execute(&sql::delete_all(&target))?;
            }
            self.execute(&sql::insert(&target, &stage))?;
        }
        self.execute(sql::COMMIT)
    }

    /// Drops every staging table and forgets the staged writes.
    fn drop_stages(&mut self) -> Result<(), AdapterError> {
        let staged = std::mem::take(&mut self.staged);
        for staged in &staged {
            self.execute(&sql::drop_stage(&sql::quote(&staged.stage)))?;
        }
        Ok(())
    }
}

impl StorageAdapter for DuckLakeAdapter {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            versioning: Versioning::Native,
            multi_table_transactions: true,
            concurrent_clients: false,
            exact_types: sql::TYPES
                .iter()
                .map(|(data_type, _)| data_type.clone())
                .collect(),
            write_modes: WRITE_MODES.to_vec(),
        }
    }

    fn write(
        &mut self,
        table: &str,
        mode: WriteMode,
        data: BatchStream,
    ) -> Result<(), AdapterError> {
        if !WRITE_MODES.contains(&mode) {
            return Err(DuckLakeError::UnsupportedMode(mode).into());
        }
        let stage = format!("mergus_stage_{}", self.next_stage);
        self.next_stage += 1;
        let schema = data.schema();
        self.stage(&stage, &schema, data)?;
        self.staged.push(Staged {
            table: table.to_owned(),
            mode,
            stage,
        });
        Ok(())
    }

    fn commit(&mut self) -> Result<VersionRef, AdapterError> {
        self.execute(sql::BEGIN)?;
        if let Err(error) = self.publish() {
            // The transaction already failed; the rollback only releases it.
            let _ = self.execute(sql::ROLLBACK);
            return Err(error);
        }
        self.drop_stages()?;
        self.latest()
    }

    fn pin_recipe(&self, table: &str, version: &VersionRef) -> Result<PinRecipe, AdapterError> {
        let unknown = || DuckLakeError::UnknownVersion(version.clone());
        let id = snapshot_id(version).ok_or_else(unknown)?;
        if !self.exists(version)? {
            return Err(unknown().into());
        }
        // Exactly these three keys, and never the catalog location (MG-REQ-0298).
        let recipe = json!({
            "format": "ducklake",
            "table": table,
            "version": id.to_string(),
        });
        Ok(PinRecipe::new(canonical_json(&recipe)))
    }

    fn read_at(&self, table: &str, version: &VersionRef) -> Result<BatchStream, AdapterError> {
        let unavailable = || AdapterError::VersionUnavailable {
            table: table.to_owned(),
            version: version.clone(),
        };
        let Some(id) = snapshot_id(version) else {
            return Err(unavailable());
        };
        if !self.exists(version)? {
            return Err(unavailable());
        }
        let query = sql::read_at(&sql::qualified(&self.alias, table));
        let batches = self.query(&query, &[SqlValue::Integer(id)])?;
        let schema = batches
            .first()
            .map_or_else(|| Arc::new(Schema::empty()), RecordBatch::schema);
        Ok(Box::new(RecordBatchIterator::new(
            batches.into_iter().map(Ok),
            schema,
        )))
    }

    fn expire(&mut self, retention: &Retention) -> Result<(), AdapterError> {
        let rows = self.query(sql::SNAPSHOTS, &[self.alias()])?;
        let all = snapshots::from_rows(&rows, sql::SNAPSHOTS)?;
        let expirable = snapshots::expirable(&all, retention, self.clock.now());
        for id in &expirable {
            self.query(sql::EXPIRE, &[self.alias(), SqlValue::Integer(*id)])?;
        }
        if !expirable.is_empty() {
            self.query(sql::CLEANUP, &[self.alias()])?;
        }
        Ok(())
    }
}
