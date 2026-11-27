//! The simulated `Database` streams a query's recorded batches one `each` call at a time.

use std::sync::Arc;

use arrow::array::{ArrayRef, Int64Array};
use arrow::record_batch::RecordBatch;
use mergus_ports::{Database, DatabaseLocation, PortError};
use mergus_sim::{DatabaseFaults, SimDatabase};

#[cfg(test)]
fn ids(values: Vec<i64>) -> RecordBatch {
    let ids: ArrayRef = Arc::new(Int64Array::from(values));
    RecordBatch::try_from_iter([("id", ids)]).unwrap()
}

#[test]
fn sim_database_calls_each_once_per_recorded_batch() {
    let database = SimDatabase::new(7, DatabaseFaults::default());
    let batches = vec![ids(vec![1]), ids(vec![2, 3]), ids(vec![4])];
    database.record_query("SELECT id FROM t", vec![], batches.clone());
    let mut session = database.connect(&DatabaseLocation::InMemory).unwrap();
    let mut seen = Vec::new();
    session
        .query_arrow_each("SELECT id FROM t", &[], &mut |batch| {
            seen.push(batch);
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, batches);

    let mut calls = 0;
    let stopped = session.query_arrow_each("SELECT id FROM t", &[], &mut |_| {
        calls += 1;
        Err(PortError::Failed("stop".to_owned()))
    });
    assert!(matches!(stopped, Err(PortError::Failed(_))));
    assert_eq!(calls, 1);
}
