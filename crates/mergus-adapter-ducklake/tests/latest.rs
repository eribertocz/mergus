//! The latest version of a catalog, on the simulated `Database`.

use std::sync::Arc;

use arrow::array::{ArrayRef, Int64Array, RecordBatch};
use mergus_adapter::VersionRef;
use mergus_adapter_ducklake::DuckLakeAdapter;
use mergus_ports::{DuckLakeCatalog, SqlValue, Timestamp};
use mergus_sim::{ClockFaults, DatabaseFaults, SimClock, SimDatabase};

const LATEST: &str = "SELECT max(snapshot_id) AS snapshot_id FROM ducklake_snapshots(?)";

#[test]
fn latest_is_none_until_the_first_commit_of_mergus() {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let catalog = DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: "metadata.ducklake".to_owned(),
        data_path: "data/".to_owned(),
    };
    let clock = Arc::new(SimClock::new(
        1,
        Timestamp::from_unix_nanos(0),
        ClockFaults::default(),
    ));
    let adapter = DuckLakeAdapter::open(&database, &catalog, clock).unwrap();
    // DuckLake makes snapshot 0 when it creates the catalog; it holds no table of Mergus.
    for (id, expected) in [(0, None), (4, Some(VersionRef::new("4")))] {
        let column: ArrayRef = Arc::new(Int64Array::from(vec![id]));
        let rows = vec![RecordBatch::try_from_iter([("snapshot_id", column)]).unwrap()];
        let alias = vec![SqlValue::Text("lake".to_owned())];
        database.record_query(LATEST, alias, rows);
        assert_eq!(adapter.latest().unwrap(), expected);
    }
}
