//! A storage adapter in memory: each commit is a version holding every table's rows.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchIterator};
use arrow::compute::concat_batches;
use arrow::datatypes::Schema;
use mergus_adapter::{
    AdapterError, BatchStream, Capabilities, PinRecipe, Retention, StorageAdapter, VersionRef,
    Versioning, WriteMode,
};

/// The rows of every table at one version.
type Tables = BTreeMap<String, RecordBatch>;

#[cfg(test)]
/// Keeps every committed version, and the writes staged since the last commit.
#[derive(Default)]
pub struct Memory {
    versions: Vec<Tables>,
    staged: Vec<(String, WriteMode, RecordBatch)>,
    /// The tables of every `write`, in order.
    pub writes: Vec<(String, WriteMode)>,
}

#[cfg(test)]
impl Memory {
    /// The rows of `table` at the newest version.
    pub fn latest(&self, table: &str) -> RecordBatch {
        self.versions.last().unwrap()[table].clone()
    }

    /// The number of commits.
    pub fn commits(&self) -> usize {
        self.versions.len()
    }
}

#[cfg(test)]
impl StorageAdapter for Memory {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            versioning: Versioning::Native,
            multi_table_transactions: true,
            concurrent_clients: false,
            exact_types: Vec::new(),
            write_modes: vec![WriteMode::Append, WriteMode::Replace],
        }
    }

    fn write(
        &mut self,
        table: &str,
        mode: WriteMode,
        data: BatchStream,
    ) -> Result<(), AdapterError> {
        let schema = data.schema();
        let batches: Vec<RecordBatch> = data.collect::<Result<_, _>>().unwrap();
        let rows = concat_batches(&schema, &batches).unwrap();
        self.writes.push((table.to_owned(), mode));
        self.staged.push((table.to_owned(), mode, rows));
        Ok(())
    }

    fn commit(&mut self) -> Result<VersionRef, AdapterError> {
        let mut tables = self.versions.last().cloned().unwrap_or_default();
        for (table, mode, rows) in std::mem::take(&mut self.staged) {
            let rows = match (mode, tables.get(&table)) {
                (WriteMode::Append, Some(old)) => {
                    concat_batches(&old.schema(), &[old.clone(), rows]).unwrap()
                }
                _ => rows,
            };
            tables.insert(table, rows);
        }
        self.versions.push(tables);
        Ok(VersionRef::new(self.versions.len().to_string()))
    }

    fn pin_recipe(&self, _table: &str, version: &VersionRef) -> Result<PinRecipe, AdapterError> {
        Ok(PinRecipe::new(version.as_str().to_owned()))
    }

    fn read_at(&self, table: &str, version: &VersionRef) -> Result<BatchStream, AdapterError> {
        let unavailable = || AdapterError::VersionUnavailable {
            table: table.to_owned(),
            version: version.clone(),
        };
        let index: usize = version.as_str().parse().map_err(|_| unavailable())?;
        let rows = self
            .versions
            .get(index.wrapping_sub(1))
            .and_then(|tables| tables.get(table))
            .ok_or_else(unavailable)?
            .clone();
        let schema: Arc<Schema> = rows.schema();
        Ok(Box::new(RecordBatchIterator::new([Ok(rows)], schema)))
    }

    fn expire(&mut self, _retention: &Retention) -> Result<(), AdapterError> {
        Ok(())
    }
}
