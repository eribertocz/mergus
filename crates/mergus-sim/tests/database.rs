//! MG-REQ-0194: the simulated `Database` answers from results recorded with the real one and
//! injects errors, deterministically for a seed.

use std::sync::Arc;

use arrow::array::{ArrayRef, Int64Array};
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use mergus_ports::{Database, DatabaseLocation, DuckLakeCatalog, PortError, SqlValue};
use mergus_sim::{DatabaseFaults, Rate, SimDatabase};

fn batch() -> Result<RecordBatch, ArrowError> {
    let ids: ArrayRef = Arc::new(Int64Array::from(vec![7]));
    RecordBatch::try_from_iter([("id", ids)])
}

#[test]
fn mg_req_0194_simulated_database_replays_recorded_results() {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let params = vec![SqlValue::Integer(7)];
    database.record_execute("INSERT INTO items VALUES (?)", params.clone(), 1);
    database.record_query("SELECT id FROM items", Vec::new(), vec![batch().unwrap()]);
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();

    let insert = "INSERT INTO items VALUES (?)";
    assert_eq!(connection.execute(insert, &params).unwrap(), 1);
    let other = [SqlValue::Integer(8)];
    assert!(matches!(
        connection.execute(insert, &other),
        Err(PortError::NotFound(_))
    ));
    let rows = connection.query_arrow("SELECT id FROM items", &[]).unwrap();
    assert_eq!(rows, vec![batch().unwrap()]);
    assert!(matches!(
        connection.execute("SELECT id FROM items", &[]),
        Err(PortError::NotFound(_))
    ));

    connection.append_arrow("items", &batch().unwrap()).unwrap();
    assert_eq!(database.appended("items"), vec![batch().unwrap()]);
    let catalog = DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: "metadata.ducklake".to_owned(),
        data_path: "data/".to_owned(),
    };
    connection.attach_ducklake(&catalog).unwrap();
    assert_eq!(database.attached(), vec![catalog]);
}

#[test]
fn mg_req_0194_simulated_database_injects_errors_for_a_seed() {
    let outcomes = |seed| {
        let faults = DatabaseFaults {
            error: Rate::per_million(500_000),
        };
        let database = SimDatabase::new(seed, faults);
        database.record_execute("DELETE FROM items", Vec::new(), 0);
        let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
        (0..64)
            .map(|_| connection.execute("DELETE FROM items", &[]).is_ok())
            .collect::<Vec<_>>()
    };
    let run = outcomes(3);
    assert_eq!(run, outcomes(3));
    assert!(run.contains(&true) && run.contains(&false));

    let always = SimDatabase::new(
        3,
        DatabaseFaults {
            error: Rate::ALWAYS,
        },
    );
    let mut connection = always.connect(&DatabaseLocation::InMemory).unwrap();
    assert!(matches!(
        connection.append_arrow("items", &batch().unwrap()),
        Err(PortError::Failed(_))
    ));
}
