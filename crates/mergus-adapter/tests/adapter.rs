//! MG-REQ-0285 and MG-REQ-0287: the six operations of the storage adapter contract, and the
//! refusal of an undeclared capability before any data is read.

use std::sync::Arc;
use std::time::Duration;

use arrow::array::{ArrayRef, Int64Array, RecordBatch, RecordBatchIterator};
use arrow::datatypes::{DataType, Field, Schema};
use mergus_adapter::{
    AdapterError, BatchStream, Capabilities, CapabilityError, PinRecipe, Requirement, Retention,
    StorageAdapter, VersionRef, Versioning, WriteMode, check_capabilities,
};

/// An in-memory adapter that implements the six operations and counts the data it touches.
#[derive(Default)]
struct Memory {
    capabilities: Capabilities,
    pending: Vec<(String, RecordBatch)>,
    committed: Vec<(String, RecordBatch)>,
    batches_read: usize,
    expired_with: Option<Retention>,
}

impl StorageAdapter for Memory {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.clone()
    }

    fn write(
        &mut self,
        table: &str,
        _mode: WriteMode,
        data: BatchStream,
    ) -> Result<(), AdapterError> {
        for batch in data {
            self.batches_read += 1;
            self.pending.push((table.to_owned(), batch?));
        }
        Ok(())
    }

    fn commit(&mut self) -> Result<VersionRef, AdapterError> {
        self.committed.append(&mut self.pending);
        Ok(VersionRef::new(self.committed.len().to_string()))
    }

    fn pin_recipe(&self, table: &str, version: &VersionRef) -> Result<PinRecipe, AdapterError> {
        Ok(PinRecipe::new(format!("{table} at {}", version.as_str())))
    }

    fn read_at(&self, table: &str, _version: &VersionRef) -> Result<BatchStream, AdapterError> {
        let batches: Vec<RecordBatch> = self
            .committed
            .iter()
            .filter(|(name, _)| name == table)
            .map(|(_, batch)| batch.clone())
            .collect();
        Ok(Box::new(RecordBatchIterator::new(
            batches.into_iter().map(Ok),
            schema(),
        )))
    }

    fn expire(&mut self, retention: &Retention) -> Result<(), AdapterError> {
        self.expired_with = Some(retention.clone());
        Ok(())
    }
}

fn schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]))
}

fn stream(values: Vec<i64>) -> BatchStream {
    let column: ArrayRef = Arc::new(Int64Array::from(values));
    let batch = RecordBatch::try_new(schema(), vec![column]);
    Box::new(RecordBatchIterator::new([batch], schema()))
}

#[test]
fn mg_req_0285_adapter_trait_has_six_operations() {
    // `Memory` implements the trait with exactly six methods, through a trait object.
    let mut memory = Memory {
        capabilities: Capabilities {
            versioning: Versioning::Native,
            write_modes: vec![WriteMode::Append],
            ..Capabilities::default()
        },
        ..Memory::default()
    };
    let adapter: &mut dyn StorageAdapter = &mut memory;
    assert_eq!(adapter.capabilities().versioning, Versioning::Native);
    adapter
        .write("orders", WriteMode::Append, stream(vec![1, 2, 3]))
        .unwrap();
    let version = adapter.commit().unwrap();
    assert_eq!(
        adapter.pin_recipe("orders", &version).unwrap().as_str(),
        "orders at 1"
    );
    let rows: usize = adapter
        .read_at("orders", &version)
        .unwrap()
        .map(|batch| batch.unwrap().num_rows())
        .sum();
    assert_eq!(rows, 3);
    let retention = Retention {
        live: vec![version.clone()],
        rollback_window: Duration::from_secs(3600),
    };
    adapter.expire(&retention).unwrap();
    assert_eq!(memory.expired_with, Some(retention));
}

/// An adapter that keeps no versions and declares two write modes, concurrent clients and
/// the type `Int64`.
fn unversioned() -> Memory {
    Memory {
        capabilities: Capabilities {
            versioning: Versioning::None,
            multi_table_transactions: false,
            concurrent_clients: true,
            exact_types: vec![DataType::Int64],
            write_modes: vec![WriteMode::Append, WriteMode::Replace],
        },
        ..Memory::default()
    }
}

#[test]
fn mg_req_0287_undeclared_capability_refused_early() {
    let memory = unversioned();
    // What the adapter declares is accepted.
    let declared = [
        Requirement::WriteMode(WriteMode::Append),
        Requirement::WriteMode(WriteMode::Replace),
        Requirement::ConcurrentClients,
        Requirement::ExactType(DataType::Int64),
    ];
    assert_eq!(
        check_capabilities(&memory.capabilities(), &declared),
        Ok(())
    );
    // Every undeclared write mode or feature is refused, all of them named, in order.
    let needs = [
        Requirement::WriteMode(WriteMode::Append),
        Requirement::WriteMode(WriteMode::MergeByKey),
        Requirement::Versioning,
        Requirement::MultiTableTransactions,
        Requirement::ExactType(DataType::Utf8),
        Requirement::WriteMode(WriteMode::ReplacePeriod),
    ];
    let error = check_capabilities(&memory.capabilities(), &needs).unwrap_err();
    assert_eq!(
        error,
        CapabilityError {
            missing: vec![
                Requirement::WriteMode(WriteMode::MergeByKey),
                Requirement::Versioning,
                Requirement::MultiTableTransactions,
                Requirement::ExactType(DataType::Utf8),
                Requirement::WriteMode(WriteMode::ReplacePeriod),
            ],
        }
    );
    assert!(error.to_string().contains("merge by key"), "{error}");
    // The check needs only the declaration: no data was read or written.
    assert_eq!(memory.batches_read, 0);
    assert!(memory.pending.is_empty() && memory.committed.is_empty());
    // Any declared versioning, native or by files, satisfies a need for versions.
    let files = Capabilities {
        versioning: Versioning::Files,
        ..Capabilities::default()
    };
    assert_eq!(
        check_capabilities(&files, &[Requirement::Versioning]),
        Ok(())
    );
}

#[test]
fn version_unavailable_names_table_and_version() {
    // MG-REQ-0252: an expired or unknown version is a typed error naming the table and the
    // version.
    let error = AdapterError::VersionUnavailable {
        table: "orders".to_owned(),
        version: VersionRef::new("42"),
    };
    assert_eq!(
        error.to_string(),
        "table `orders` has no readable version `42`"
    );
    let AdapterError::VersionUnavailable { table, version } = error else {
        panic!("not a `VersionUnavailable` error");
    };
    assert_eq!((table.as_str(), version.as_str()), ("orders", "42"));
}
