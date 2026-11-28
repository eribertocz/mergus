//! MG-REQ-0633, MG-REQ-0180 and MG-REQ-0643 on the real DuckDB and file system: the statements
//! the simulated tests record are the ones DuckDB runs. Parquet is built into the bundled
//! DuckDB, so no extension is needed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{ArrayRef, Decimal128Array, Int64Array, TimestampMicrosecondArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::{Boundary, EngineError, Segment, Table, Transport, run_segment};
use mergus_ports::real::{RealDatabase, RealFileSystem, RealRandom};
use mergus_ports::{Database, DatabaseLocation, FileSystem, Random, SqlValue};

const MADRID: &str = "Europe/Madrid";
const MACRO: &str = "CREATE MACRO twice(a) AS a * 2;";

/// Returns a new, empty directory under the target directory of the tests.
#[cfg(test)]
fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "mergus-engine-duckdb-{:016x}",
        RealRandom::new().next_u64()
    ));
    RealFileSystem.create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
fn events_schema(id: DataType) -> SchemaRef {
    let zoned = DataType::Timestamp(TimeUnit::Microsecond, Some(MADRID.into()));
    Arc::new(Schema::new(vec![
        Field::new("id", id, true),
        Field::new("amount", DataType::Decimal128(14, 2), true),
        Field::new("at", zoned, true),
    ]))
}

#[cfg(test)]
fn events() -> RecordBatch {
    let amounts = Decimal128Array::from(vec![Some(123_456_789_012), None])
        .with_precision_and_scale(14, 2)
        .unwrap();
    let at = TimestampMicrosecondArray::from(vec![Some(1_700_000_000_000_000), None])
        .with_timezone(MADRID);
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(vec![1, 2])),
        Arc::new(amounts),
        Arc::new(at),
    ];
    RecordBatch::try_new(events_schema(DataType::Int64), columns).unwrap()
}

#[cfg(test)]
fn table(name: &str, schema: SchemaRef, transport: Transport, path: &Path) -> Table {
    let boundary = Boundary {
        transport,
        path: path.to_path_buf(),
    };
    Table {
        name: name.to_owned(),
        schema,
        boundary,
    }
}

#[cfg(test)]
fn segment(sql: &str, inputs: Vec<Table>, output: Table) -> Segment {
    Segment {
        sql: sql.to_owned(),
        params: vec![],
        macros: vec![MACRO.to_owned()],
        inputs,
        output,
    }
}

#[cfg(test)]
fn read_ipc(path: &Path) -> (SchemaRef, Vec<RecordBatch>) {
    let bytes = RealFileSystem.read(path).unwrap();
    let reader = FileReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
    let schema = reader.schema();
    (schema, reader.map(|batch| batch.unwrap()).collect())
}

/// Writes the events as Arrow IPC in a new scratch directory, then runs a segment that reads
/// them from shared memory with a macro of the session and writes them as Parquet. Returns
/// the directory and the Parquet file.
#[cfg(test)]
fn staged() -> (PathBuf, PathBuf) {
    let dir = scratch();
    let source = dir.join("events.arrow");
    let mut bytes = Vec::new();
    let mut writer = FileWriter::try_new(&mut bytes, &events().schema()).unwrap();
    writer.write(&events()).unwrap();
    writer.finish().unwrap();
    drop(writer);
    RealFileSystem.write(&source, &bytes).unwrap();
    let schema = events_schema(DataType::Int64);
    let input = table("events", schema.clone(), Transport::SharedMemory, &source);
    let staged = dir.join("staged.parquet");
    let output = table("staged", schema, Transport::Parquet, &staged);
    let sql = "SELECT twice(id) // 2 AS id, amount, \"at\" FROM events";
    run(&dir, &segment(sql, vec![input], output)).unwrap();
    assert!(
        !RealFileSystem
            .exists(&dir.join("staged.parquet.duckdb"))
            .unwrap()
    );
    (dir, staged)
}

#[cfg(test)]
fn run(dir: &Path, segment: &Segment) -> Result<(), EngineError> {
    run_segment(
        &RealDatabase::new(&dir.join("cache")),
        &RealFileSystem,
        segment,
    )
}

#[test]
fn mg_req_0633_boundaries_round_trip_on_duckdb() {
    let (dir, staged) = staged();
    let schema = events_schema(DataType::Int64);
    let input = table("staged", schema.clone(), Transport::Parquet, &staged);
    let out = dir.join("out.arrow");
    let output = table("final", schema.clone(), Transport::SharedMemory, &out);
    let sql = "SELECT id, amount, \"at\" FROM staged ORDER BY id";
    run(&dir, &segment(sql, vec![input], output)).unwrap();
    let (read_schema, batches) = read_ipc(&out);
    assert_eq!(read_schema, schema);
    assert_eq!(batches, vec![events()]);
}

#[test]
fn mg_req_0633_empty_output_keeps_its_columns_on_duckdb() {
    let (dir, staged) = staged();
    let schema = events_schema(DataType::Int64);
    let input = table("staged", schema.clone(), Transport::Parquet, &staged);
    let empty = dir.join("empty.parquet");
    let output = table("empty", schema.clone(), Transport::Parquet, &empty);
    let none = "SELECT id, amount, \"at\" FROM staged WHERE id < 0";
    run(&dir, &segment(none, vec![input], output)).unwrap();
    let input = table("empty", schema.clone(), Transport::Parquet, &empty);
    let out = dir.join("empty.arrow");
    let output = table("final", schema.clone(), Transport::SharedMemory, &out);
    run(&dir, &segment("SELECT * FROM empty", vec![input], output)).unwrap();
    let (read_schema, batches) = read_ipc(&out);
    assert_eq!(read_schema, schema);
    assert!(batches.iter().all(|batch| batch.num_rows() == 0));
}

#[test]
fn mg_req_0180_parquet_input_of_another_type_fails_on_duckdb() {
    let (dir, staged) = staged();
    let input = table(
        "staged",
        events_schema(DataType::Int32),
        Transport::Parquet,
        &staged,
    );
    let out = dir.join("narrow.arrow");
    let output = table(
        "final",
        events_schema(DataType::Int64),
        Transport::SharedMemory,
        &out,
    );
    let sql = "SELECT id, amount, \"at\" FROM staged";
    let error = run(&dir, &segment(sql, vec![input], output)).unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");
    assert!(matches!(&error, EngineError::Unrepresentable { column, .. } if column == "id"));
    assert!(!RealFileSystem.exists(&out).unwrap());
}

#[test]
fn mg_req_0180_sessions_have_no_icu_on_duckdb() {
    let dir = scratch();
    let database = RealDatabase::new(&dir.join("cache"));
    let mut session = database.connect(&DatabaseLocation::InMemory).unwrap();
    let loaded = session
        .query_arrow(
            "SELECT extension_name FROM duckdb_extensions() WHERE loaded AND extension_name = ?",
            &[SqlValue::Text("icu".to_owned())],
        )
        .unwrap();
    assert!(loaded.iter().all(|batch| batch.num_rows() == 0));
}

#[test]
fn mg_req_0180_exact_widening_is_accepted_on_duckdb() {
    let dir = scratch();
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, true),
        Field::new("amount", DataType::Decimal128(14, 1), true),
    ]));
    let out = dir.join("widened.arrow");
    let output = table("widened", schema.clone(), Transport::SharedMemory, &out);
    let sql = "SELECT 1 AS id, 1.5 AS amount";
    run(&dir, &segment(sql, vec![], output)).unwrap();
    let (read_schema, batches) = read_ipc(&out);
    assert_eq!(read_schema, schema);
    assert_eq!(batches.iter().map(RecordBatch::num_rows).sum::<usize>(), 1);

    let narrow = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, true)]));
    let out = dir.join("narrowed.arrow");
    let output = table("narrowed", narrow, Transport::SharedMemory, &out);
    let error = run(&dir, &segment("SELECT 1::BIGINT AS id", vec![], output)).unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");
}

#[test]
fn mg_req_0633_duckdb_streams_a_result_in_several_batches() {
    let dir = scratch();
    let database = RealDatabase::new(&dir.join("cache"));
    let mut session = database.connect(&DatabaseLocation::InMemory).unwrap();
    let mut calls = 0;
    let mut rows = 0;
    session
        .query_arrow_each(
            "SELECT range AS id FROM range(?)",
            &[SqlValue::Integer(10_000)],
            &mut |batch| {
                calls += 1;
                rows += batch.num_rows();
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(rows, 10_000);
    assert!(calls > 1, "{calls} batches");
}

#[test]
fn mg_req_0710_rejected_sql_names_the_model_and_duckdb_message() {
    let dir = scratch();
    let schema = events_schema(DataType::Int64);
    let out = dir.join("out.arrow");
    let output = table("orders", schema, Transport::SharedMemory, &out);
    let sql = "SELECT id, amount, \"at\" FROM no_such_table";
    let error = run(&dir, &segment(sql, vec![], output)).unwrap_err();
    assert_eq!(error.code(), Some("MG-0103"));
    let EngineError::Sql { model, message } = &error else {
        panic!("not a SQL failure: {error}");
    };
    assert_eq!(model, "orders");
    assert!(message.contains("Catalog Error: "), "{message}");
    assert!(error.to_string().contains("orders"), "{error}");
}
