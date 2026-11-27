//! MG-REQ-0643: every DuckDB session that runs a segment loads the project's macros first.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arrow::array::{ArrayRef, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::{Boundary, Segment, Table, Transport, run_segment};
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, FileSystem, PortError, SqlValue,
};
use mergus_sim::{DatabaseFaults, SimDatabase, SimFileSystem};

const SQL: &str = "SELECT id FROM orders";
const MACROS: [&str; 2] = [
    "CREATE MACRO safe_div(a, b) AS CASE WHEN b = 0 THEN NULL ELSE a / b END;",
    "CREATE MACRO clean(t) AS TABLE SELECT * FROM query_table(t) WHERE status <> 'VOID';",
];
const CREATE_OUTPUT: &str = "CREATE TABLE \"mergus_output\" (\"id\" BIGINT)";
const LOADED_ICU: &str =
    "SELECT extension_name FROM duckdb_extensions() WHERE loaded AND extension_name = ?";
const COPY: &str = "COPY \"mergus_output\" TO ? (FORMAT parquet)";

/// The answer of `DESCRIBE` for these columns and DuckDB types.
#[cfg(test)]
fn describe(columns: &[(&str, &str)]) -> Vec<RecordBatch> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("column_name", DataType::Utf8, true),
        Field::new("column_type", DataType::Utf8, true),
    ]));
    let names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
    let types: Vec<&str> = columns.iter().map(|(_, data_type)| *data_type).collect();
    let arrays: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(names)),
        Arc::new(StringArray::from(types)),
    ];
    vec![RecordBatch::try_new(schema, arrays).unwrap()]
}

#[cfg(test)]
fn database() -> SimDatabase {
    let database = SimDatabase::new(7, DatabaseFaults::default());
    database.record_query(LOADED_ICU, vec![SqlValue::Text("icu".to_owned())], vec![]);
    for text in MACROS {
        database.record_execute(text, vec![], 0);
    }
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, true)]));
    let ids: ArrayRef = Arc::new(Int64Array::from(vec![1, 2]));
    let batch = RecordBatch::try_new(schema, vec![ids]).unwrap();
    database.record_query(
        format!("DESCRIBE {SQL}"),
        vec![],
        describe(&[("id", "BIGINT")]),
    );
    database.record_query(SQL, vec![], vec![batch]);
    database.record_execute(CREATE_OUTPUT, vec![], 0);
    let path = SqlValue::Text("/zone/work/a.parquet".to_owned());
    database.record_execute(COPY, vec![path], 2);
    database
}

#[cfg(test)]
fn segment() -> Segment {
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, true)]));
    let boundary = Boundary {
        transport: Transport::Parquet,
        path: PathBuf::from("/zone/work/a.parquet"),
    };
    Segment {
        sql: SQL.to_owned(),
        params: vec![],
        macros: MACROS.iter().map(|text| (*text).to_owned()).collect(),
        inputs: vec![],
        output: Table {
            name: "orders_out".to_owned(),
            schema,
            boundary,
        },
    }
}

/// A database that counts its connections.
struct Counted {
    inner: SimDatabase,
    connections: AtomicUsize,
}

impl Database for Counted {
    fn connect(
        &self,
        location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        self.connections.fetch_add(1, Ordering::SeqCst);
        self.inner.connect(location)
    }
}

#[test]
fn mg_req_0643_macros_loaded_in_each_session() {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/zone/work")).unwrap();
    let database = Counted {
        inner: database(),
        connections: AtomicUsize::new(0),
    };
    run_segment(&database, &files, &segment()).unwrap();
    run_segment(&database, &files, &segment()).unwrap();
    assert_eq!(database.connections.load(Ordering::SeqCst), 2);
    let session = [MACROS[0], MACROS[1], CREATE_OUTPUT, COPY];
    let expected: Vec<String> = session
        .iter()
        .chain(session.iter())
        .map(|sql| (*sql).to_owned())
        .collect();
    let executed: Vec<String> = database
        .inner
        .executed()
        .into_iter()
        .map(|(sql, _)| sql)
        .collect();
    assert_eq!(executed, expected);
}
