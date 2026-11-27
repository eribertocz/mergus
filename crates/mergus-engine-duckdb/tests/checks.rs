//! MG-REQ-0180: the checks of a segment before it reads or writes a row, with the simulated
//! ports.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{ArrayRef, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::{Boundary, EngineError, Segment, Table, Transport, run_segment};
use mergus_ports::{FileSystem, SqlValue};
use mergus_sim::{DatabaseFaults, SimDatabase, SimFileSystem};

const SQL: &str = "SELECT id FROM orders";
const LOADED_ICU: &str =
    "SELECT extension_name FROM duckdb_extensions() WHERE loaded AND extension_name = ?";
const MACRO: &str = "CREATE MACRO twice(a) AS a * 2;";

#[cfg(test)]
fn ids(data_type: DataType) -> SchemaRef {
    Arc::new(Schema::new(vec![Field::new("id", data_type, true)]))
}

#[cfg(test)]
fn table(name: &str, schema: SchemaRef, transport: Transport, path: &str) -> Table {
    let boundary = Boundary {
        transport,
        path: PathBuf::from(path),
    };
    Table {
        name: name.to_owned(),
        schema,
        boundary,
    }
}

#[cfg(test)]
fn segment(output: Table, inputs: Vec<Table>) -> Segment {
    Segment {
        sql: SQL.to_owned(),
        params: vec![],
        macros: vec![MACRO.to_owned()],
        inputs,
        output,
    }
}

#[cfg(test)]
fn arrow_output() -> Table {
    let schema = ids(DataType::Int64);
    table(
        "orders_out",
        schema,
        Transport::SharedMemory,
        "/work/out.arrow",
    )
}

/// A database whose session has `icu` loaded when `icu` is set, and that describes the
/// step's result with `described`, the DuckDB type of `id`.
#[cfg(test)]
fn database(icu: bool, described: &str) -> SimDatabase {
    let database = SimDatabase::new(7, DatabaseFaults::default());
    let name: ArrayRef = Arc::new(StringArray::from(vec!["icu"]));
    let loaded = RecordBatch::try_from_iter([("extension_name", name)]).unwrap();
    let answer = if icu { vec![loaded] } else { vec![] };
    database.record_query(LOADED_ICU, vec![SqlValue::Text("icu".to_owned())], answer);
    database.record_execute(MACRO, vec![], 0);
    let names: ArrayRef = Arc::new(StringArray::from(vec!["id"]));
    let types: ArrayRef = Arc::new(StringArray::from(vec![described]));
    let description =
        RecordBatch::try_from_iter([("column_name", names), ("column_type", types)]).unwrap();
    database.record_query(format!("DESCRIBE {SQL}"), vec![], vec![description]);
    database.record_query(SQL, vec![], vec![]);
    database
}

#[cfg(test)]
fn work() -> SimFileSystem {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/work")).unwrap();
    files
}

#[test]
fn mg_req_0180_zero_rows_of_another_type_fail() {
    let files = work();
    let wrong = database(false, "INTEGER");
    let error = run_segment(&wrong, &files, &segment(arrow_output(), vec![])).unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");
    assert!(
        matches!(&error, EngineError::Unrepresentable { table, column, .. }
        if table == "orders_out" && column == "id")
    );
    assert!(!files.exists(Path::new("/work/out.arrow")).unwrap());

    let right = database(false, "BIGINT");
    run_segment(&right, &files, &segment(arrow_output(), vec![])).unwrap();
    assert!(files.exists(Path::new("/work/out.arrow")).unwrap());
}

#[test]
fn mg_req_0180_icu_in_the_session_fails() {
    let files = work();
    let database = database(true, "BIGINT");
    let error = run_segment(&database, &files, &segment(arrow_output(), vec![])).unwrap_err();
    assert_eq!(error.code(), Some("MG-0102"), "{error}");
    assert!(matches!(error, EngineError::ZoneExtensionLoaded));
    assert!(database.executed().is_empty());
}

#[test]
fn mg_req_0180_undeclarable_output_fails_before_the_session() {
    let files = work();
    let database = database(false, "BIGINT");
    let half = table(
        "orders_out",
        ids(DataType::Float16),
        Transport::Parquet,
        "/w/o.parquet",
    );
    let error = run_segment(&database, &files, &segment(half, vec![])).unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");
    assert!(
        matches!(&error, EngineError::Unrepresentable { table, column, .. }
        if table == "orders_out" && column == "id")
    );

    let twice = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, true),
        Field::new("id", DataType::Utf8, true),
    ]));
    let output = table(
        "orders_out",
        twice,
        Transport::SharedMemory,
        "/work/out.arrow",
    );
    let error = run_segment(&database, &files, &segment(output, vec![])).unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");
    assert!(database.executed().is_empty());
}

#[test]
fn mg_req_0633_output_table_never_takes_an_input_name() {
    let files = work();
    let database = database(false, "BIGINT");
    let input = table(
        "mergus_output",
        ids(DataType::Int64),
        Transport::Parquet,
        "/w/i.parquet",
    );
    let path = SqlValue::Text("/w/i.parquet".to_owned());
    database.record_execute("SET VARIABLE mergus_input_0 = ?", vec![path], 0);
    let file = "read_parquet(getvariable('mergus_input_0'))";
    let names: ArrayRef = Arc::new(StringArray::from(vec!["id"]));
    let types: ArrayRef = Arc::new(StringArray::from(vec!["BIGINT"]));
    let description =
        RecordBatch::try_from_iter([("column_name", names), ("column_type", types)]).unwrap();
    database.record_query(
        format!("DESCRIBE SELECT * FROM {file}"),
        vec![],
        vec![description],
    );
    let view = "CREATE VIEW \"mergus_output\" AS SELECT \"id\" FROM \
                read_parquet(getvariable('mergus_input_0'))";
    database.record_execute(view, vec![], 0);
    let create = "CREATE TABLE \"mergus_output_\" (\"id\" BIGINT)";
    database.record_execute(create, vec![], 0);
    let copy = "COPY \"mergus_output_\" TO ? (FORMAT parquet)";
    let out = SqlValue::Text("/w/o.parquet".to_owned());
    database.record_execute(copy, vec![out.clone()], 0);
    let output = table(
        "orders_out",
        ids(DataType::Int64),
        Transport::Parquet,
        "/w/o.parquet",
    );
    run_segment(&database, &files, &segment(output, vec![input])).unwrap();
    let executed = database.executed();
    assert!(executed.contains(&(create.to_owned(), vec![])));
    assert!(executed.contains(&(copy.to_owned(), vec![out])));
}
