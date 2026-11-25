//! MG-REQ-0286 and MG-REQ-0288 on a real DuckLake catalog. The test never uses the network:
//! it runs only when the signed `ducklake` extension is already in the user cache, and
//! returns at once otherwise.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{ArrayRef, AsArray, Int64Array, RecordBatch, RecordBatchIterator};
use arrow::datatypes::{DataType, Field, Int64Type, Schema};
use mergus_adapter::{AdapterError, BatchStream, Retention, StorageAdapter, VersionRef, WriteMode};
use mergus_adapter_ducklake::DuckLakeAdapter;
use mergus_ports::real::{RealClock, RealDatabase, RealEnvironment, RealFileSystem, RealRandom};
use mergus_ports::{Database, DatabaseLocation, DuckLakeCatalog, Environment, FileSystem, Random};

/// Returns the real database when `ducklake` is in its cache.
#[cfg(test)]
fn cached_database() -> Option<RealDatabase> {
    let database = RealDatabase::new(&RealEnvironment.cache_dir()?);
    let mut connection = database.connect(&DatabaseLocation::InMemory).ok()?;
    let sql = "SELECT library_version || '/' || platform FROM pragma_version(), pragma_platform()";
    let batches = connection.query_arrow(sql, &[]).ok()?;
    let path = batches
        .first()?
        .column(0)
        .as_string::<i32>()
        .value(0)
        .to_owned();
    let file = database
        .extension_dir()
        .join(path)
        .join("ducklake.duckdb_extension");
    RealFileSystem.exists(&file).ok()?.then_some(database)
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
    let Some(database) = cached_database() else {
        return;
    };
    let dir = scratch();
    let catalog = DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: dir.join("metadata.ducklake").display().to_string(),
        data_path: dir.join("data").display().to_string(),
    };
    let mut adapter =
        DuckLakeAdapter::open(&database, &catalog, Arc::new(RealClock::new())).unwrap();
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
