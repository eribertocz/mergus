//! MG-REQ-0286 and MG-REQ-0288 on a real DuckLake catalog. The real `Database` installs the
//! signed `ducklake` extension into the user cache the first time; the test returns at once
//! when the extension is not cached and cannot be downloaded.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{ArrayRef, AsArray, Int64Array, RecordBatch, RecordBatchIterator};
use arrow::datatypes::{DataType, Field, Int64Type, Schema};
use mergus_adapter::{AdapterError, BatchStream, Retention, StorageAdapter, VersionRef, WriteMode};
use mergus_adapter_ducklake::{DuckLakeAdapter, DuckLakeError};
use mergus_ports::real::{RealClock, RealDatabase, RealEnvironment, RealFileSystem, RealRandom};
use mergus_ports::{DuckLakeCatalog, Environment, FileSystem, PortError, Random};

/// Opens the adapter on `catalog` with the real `Database`, which installs `ducklake` into
/// the user cache the first time (MG-REQ-0194). Returns `None` when the extension is not
/// cached and cannot be downloaded.
#[cfg(test)]
fn open(catalog: &DuckLakeCatalog) -> Option<DuckLakeAdapter> {
    let database = RealDatabase::new(&RealEnvironment.cache_dir()?);
    match DuckLakeAdapter::open(&database, catalog, Arc::new(RealClock::new())) {
        Ok(adapter) => Some(adapter),
        Err(AdapterError::Storage(error))
            if matches!(
                error.downcast_ref::<DuckLakeError>(),
                Some(DuckLakeError::Port(PortError::ExtensionUnavailable { .. }))
            ) =>
        {
            None
        }
        Err(error) => panic!("{error}"),
    }
}

/// Returns a new, empty directory under the target directory of the tests.
#[cfg(test)]
fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "mergus-ducklake-{:016x}",
        RealRandom::new().next_u64()
    ));
    RealFileSystem.create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
fn orders(ids: Vec<i64>) -> BatchStream {
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, true)]));
    let column: ArrayRef = Arc::new(Int64Array::from(ids));
    let batch = RecordBatch::try_new(Arc::clone(&schema), vec![column]);
    Box::new(RecordBatchIterator::new([batch], schema))
}

#[cfg(test)]
fn ids(adapter: &DuckLakeAdapter, version: &VersionRef) -> Vec<i64> {
    let mut ids: Vec<i64> = adapter
        .read_at("orders", version)
        .unwrap()
        .flat_map(|batch| {
            let batch = batch.unwrap();
            batch
                .column(0)
                .as_primitive::<Int64Type>()
                .values()
                .to_vec()
        })
        .collect();
    ids.sort_unstable();
    ids
}

#[test]
fn ducklake_round_trip_keeps_uncommitted_rows_and_live_versions() {
    let dir = scratch();
    let catalog = DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: dir.join("metadata.ducklake").display().to_string(),
        data_path: dir.join("data").display().to_string(),
    };
    let Some(mut adapter) = open(&catalog) else {
        return;
    };
    adapter
        .write("orders", WriteMode::Append, orders(vec![1, 2]))
        .unwrap();
    let first = adapter.commit().unwrap();

    // MG-REQ-0286: rows written and not committed are not in the published version.
    adapter
        .write("orders", WriteMode::Append, orders(vec![3]))
        .unwrap();
    assert_eq!(ids(&adapter, &first), [1, 2]);
    let second = adapter.commit().unwrap();
    assert_eq!(ids(&adapter, &second), [1, 2, 3]);
    adapter
        .write("orders", WriteMode::Replace, orders(vec![4]))
        .unwrap();
    let third = adapter.commit().unwrap();
    assert_eq!(ids(&adapter, &third), [4]);

    // MG-REQ-0288: a live version survives; an unreferenced old one is expired.
    let retention = Retention {
        live: vec![first.clone()],
        rollback_window: Duration::ZERO,
    };
    adapter.expire(&retention).unwrap();
    assert_eq!(ids(&adapter, &first), [1, 2]);
    assert_eq!(ids(&adapter, &third), [4]);
    let error = adapter.read_at("orders", &second).err().unwrap();
    assert!(
        matches!(error, AdapterError::VersionUnavailable { .. }),
        "{error}"
    );
}
