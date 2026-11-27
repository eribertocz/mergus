//! The behavior of a segment, with the simulated `Database` and `FileSystem` ports.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, Decimal128Array, Int64Array, StringArray, TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::{
    Boundary, EngineError, Input, MEMORY_BUDGET, Segment, Transport, run_segment, table_hash,
};
use mergus_ports::{FileSystem, SqlValue};
use mergus_sim::{DatabaseFaults, SimDatabase, SimFileSystem};

const SQL: &str = "SELECT id, name FROM orders WHERE id > ?";
const MACROS: [&str; 2] = [
    "CREATE MACRO safe_div(a, b) AS CASE WHEN b = 0 THEN NULL ELSE a / b END;",
    "CREATE MACRO clean(t) AS TABLE SELECT * FROM query_table(t) WHERE status <> 'VOID';",
];
const CREATE_OUTPUT: &str = "CREATE TABLE \"mergus_output\" (\"id\" BIGINT, \"name\" VARCHAR)";
const COPY: &str = "COPY \"mergus_output\" TO ? (FORMAT parquet)";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn rows(ids: &[i64], names: &[&str]) -> Result<RecordBatch> {
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(ids.to_vec())),
        Arc::new(StringArray::from(names.to_vec())),
    ];
    Ok(RecordBatch::try_new(rows_schema(), columns)?)
}

fn rows_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, true),
        Field::new("name", DataType::Utf8, true),
    ]))
}

fn boundary(transport: Transport, path: &str) -> Boundary {
    Boundary {
        transport,
        path: PathBuf::from(path),
    }
}

fn segment(output: Boundary, inputs: Vec<Input>) -> Segment {
    Segment {
        sql: SQL.to_owned(),
        params: vec![SqlValue::Integer(1)],
        macros: MACROS.iter().map(|text| (*text).to_owned()).collect(),
        inputs,
        output,
    }
}

/// A database that answers the segment's statements, and the macros.
fn database() -> Result<SimDatabase> {
    let database = SimDatabase::new(7, DatabaseFaults::default());
    for text in MACROS {
        database.record_execute(text, vec![], 0);
    }
    database.record_query(
        SQL,
        vec![SqlValue::Integer(1)],
        vec![rows(&[2, 3], &["b", "c"])?],
    );
    database.record_execute(CREATE_OUTPUT, vec![], 0);
    Ok(database)
}

fn write_ipc(files: &SimFileSystem, path: &str, batch: &RecordBatch) -> Result<()> {
    let mut bytes = Vec::new();
    let mut writer = FileWriter::try_new(&mut bytes, &batch.schema())?;
    writer.write(batch)?;
    writer.finish()?;
    drop(writer);
    files.create_dir_all(Path::new("/work"))?;
    files.write(Path::new(path), &bytes)?;
    Ok(())
}

fn executed(database: &SimDatabase) -> Vec<String> {
    database
        .executed()
        .into_iter()
        .map(|(sql, _)| sql)
        .collect()
}

#[test]
fn mg_req_0633_transport_falls_back_to_parquet() {
    assert_eq!(Transport::for_size(MEMORY_BUDGET), Transport::SharedMemory);
    assert_eq!(Transport::for_size(MEMORY_BUDGET + 1), Transport::Parquet);

    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/work")).unwrap();
    let database = database().unwrap();
    let arrow = boundary(Transport::SharedMemory, "/work/orders.arrow");
    run_segment(&database, &files, &segment(arrow, vec![])).unwrap();
    let bytes = files.read(Path::new("/work/orders.arrow")).unwrap();
    let read: Vec<RecordBatch> = FileReader::try_new(Cursor::new(bytes), None)
        .unwrap()
        .map(|batch| batch.unwrap())
        .collect();
    assert_eq!(read, vec![rows(&[2, 3], &["b", "c"]).unwrap()]);
    assert!(!executed(&database).iter().any(|sql| sql == COPY));

    let path = SqlValue::Text("/zone/work/orders.parquet".to_owned());
    database.record_execute(COPY, vec![path.clone()], 2);
    let parquet = boundary(Transport::Parquet, "/zone/work/orders.parquet");
    run_segment(&database, &files, &segment(parquet, vec![])).unwrap();
    assert!(database.executed().contains(&(COPY.to_owned(), vec![path])));
    assert!(
        !files
            .exists(Path::new("/zone/work/orders.parquet"))
            .unwrap()
    );
}

#[test]
fn mg_req_0633_parquet_input_is_read_lazily() {
    let files = SimFileSystem::new();
    let database = database().unwrap();
    let path = SqlValue::Text("/zone/work/orders.parquet".to_owned());
    let set = "SET VARIABLE mergus_input_0 = ?";
    let view =
        "CREATE VIEW \"orders\" AS SELECT * FROM read_parquet(getvariable('mergus_input_0'))";
    database.record_execute(set, vec![path.clone()], 0);
    database.record_execute(view, vec![], 0);
    let input = Input {
        name: "orders".to_owned(),
        schema: rows_schema(),
        boundary: boundary(Transport::Parquet, "/zone/work/orders.parquet"),
    };
    files.create_dir_all(Path::new("/work")).unwrap();
    let output = boundary(Transport::SharedMemory, "/work/out.arrow");
    run_segment(&database, &files, &segment(output, vec![input])).unwrap();
    assert!(database.executed().contains(&(set.to_owned(), vec![path])));
    assert!(executed(&database).contains(&view.to_owned()));
    assert!(database.appended("orders").is_empty());
}

#[test]
fn mg_req_0638_hashes_independent_of_row_order() {
    let ordered = [rows(&[1, 2, 3], &["a", "b", "c"]).unwrap()];
    let shuffled = [
        rows(&[3, 1], &["c", "a"]).unwrap(),
        rows(&[2], &["b"]).unwrap(),
    ];
    let hash = table_hash(&ordered).unwrap();
    assert_eq!(hash, table_hash(&shuffled).unwrap());
    assert_eq!(hash.len(), 64);
    let changed = [rows(&[1, 2, 3], &["a", "b", "d"]).unwrap()];
    assert_ne!(hash, table_hash(&changed).unwrap());
}

#[test]
fn mg_req_0643_macros_loaded_in_each_session() {
    let files = SimFileSystem::new();
    let database = database().unwrap();
    let path = SqlValue::Text("/zone/work/a.parquet".to_owned());
    database.record_execute(COPY, vec![path], 2);
    let output = boundary(Transport::Parquet, "/zone/work/a.parquet");
    run_segment(&database, &files, &segment(output.clone(), vec![])).unwrap();
    run_segment(&database, &files, &segment(output, vec![])).unwrap();
    let session = [MACROS[0], MACROS[1], CREATE_OUTPUT, COPY];
    let expected: Vec<String> = session
        .iter()
        .chain(session.iter())
        .map(|sql| (*sql).to_owned())
        .collect();
    assert_eq!(executed(&database), expected);
}

/// Writes `values` as a column `amount` of type `DECIMAL(precision, scale)` and returns the
/// input `amounts` that declares the column as `to`.
fn decimal_input(
    files: &SimFileSystem,
    values: Vec<Option<i128>>,
    (precision, scale): (u8, i8),
    to: DataType,
) -> Result<Input> {
    let array = Decimal128Array::from(values).with_precision_and_scale(precision, scale)?;
    let from = DataType::Decimal128(precision, scale);
    let schema = Arc::new(Schema::new(vec![Field::new("amount", from, true)]));
    let batch = RecordBatch::try_new(schema, vec![Arc::new(array)])?;
    write_ipc(files, "/work/amounts.arrow", &batch)?;
    Ok(Input {
        name: "amounts".to_owned(),
        schema: Arc::new(Schema::new(vec![Field::new("amount", to, true)])),
        boundary: boundary(Transport::SharedMemory, "/work/amounts.arrow"),
    })
}

fn run_with(
    database: &SimDatabase,
    files: &SimFileSystem,
    input: Input,
) -> std::result::Result<(), EngineError> {
    let output = boundary(Transport::SharedMemory, "/work/out.arrow");
    run_segment(database, files, &segment(output, vec![input]))
}

/// Returns whether `result` is `MG-0101` on the column `amount` of `amounts`, with nothing
/// appended.
fn unrepresentable(result: std::result::Result<(), EngineError>, database: &SimDatabase) -> bool {
    let failed = matches!(&result, Err(error @ EngineError::Unrepresentable { input, column, .. })
        if error.code() == Some("MG-0101") && input == "amounts" && column == "amount");
    failed && database.appended("amounts").is_empty()
}

#[test]
fn mg_req_0180_unrepresentable_value_fails() {
    let files = SimFileSystem::new();
    let database = database().unwrap();

    // A DECIMAL overflow: 1234567890.12 does not fit DECIMAL(8, 2).
    let input = decimal_input(
        &files,
        vec![Some(1), Some(123_456_789_012)],
        (12, 2),
        DataType::Decimal128(8, 2),
    );
    assert!(unrepresentable(
        run_with(&database, &files, input.unwrap()),
        &database
    ));

    // A lost digit: 1.25 is not 1.2 or 1.3.
    let input = decimal_input(
        &files,
        vec![Some(125)],
        (10, 2),
        DataType::Decimal128(10, 1),
    );
    assert!(unrepresentable(
        run_with(&database, &files, input.unwrap()),
        &database
    ));

    // A HUGEINT beyond BIGINT: DuckDB gives HUGEINT to Arrow as DECIMAL(38, 0).
    let input = decimal_input(&files, vec![Some(1_i128 << 70)], (38, 0), DataType::Int64);
    assert!(unrepresentable(
        run_with(&database, &files, input.unwrap()),
        &database
    ));
}

#[test]
fn mg_req_0180_exact_values_are_kept() {
    let files = SimFileSystem::new();
    let database = database().unwrap();
    database.record_execute(
        "CREATE TABLE \"amounts\" (\"amount\" DECIMAL(14, 2))",
        vec![],
        0,
    );
    let input = decimal_input(
        &files,
        vec![Some(123_456_789_012), None],
        (12, 2),
        DataType::Decimal128(14, 2),
    )
    .unwrap();
    run_with(&database, &files, input).unwrap();
    let appended = database.appended("amounts");
    let expected = Decimal128Array::from(vec![Some(123_456_789_012), None])
        .with_precision_and_scale(14, 2)
        .unwrap();
    assert_eq!(appended.len(), 1);
    assert_eq!(appended[0].column(0).to_data(), expected.to_data());

    database.record_execute("CREATE TABLE \"amounts\" (\"amount\" BIGINT)", vec![], 0);
    let input = decimal_input(&files, vec![Some(42), None], (38, 0), DataType::Int64).unwrap();
    run_with(&database, &files, input).unwrap();
    let last = database.appended("amounts").pop().unwrap();
    assert_eq!(
        last.column(0).to_data(),
        Int64Array::from(vec![Some(42), None]).to_data()
    );
}

#[test]
fn mg_req_0180_timestamps_keep_their_time_zone() {
    let files = SimFileSystem::new();
    let database = database().unwrap();
    let zoned = DataType::Timestamp(TimeUnit::Microsecond, Some("Europe/Madrid".into()));
    let array = TimestampMicrosecondArray::from(vec![Some(1_700_000_000_000_000)])
        .with_timezone("Europe/Madrid");
    let schema = Arc::new(Schema::new(vec![Field::new("at", zoned.clone(), true)]));
    let batch = RecordBatch::try_new(schema, vec![Arc::new(array)]).unwrap();
    write_ipc(&files, "/work/events.arrow", &batch).unwrap();
    let naive = DataType::Timestamp(TimeUnit::Microsecond, None);
    let input = |data_type: DataType| Input {
        name: "events".to_owned(),
        schema: Arc::new(Schema::new(vec![Field::new("at", data_type, true)])),
        boundary: boundary(Transport::SharedMemory, "/work/events.arrow"),
    };
    let output = boundary(Transport::SharedMemory, "/work/out.arrow");
    let error = run_segment(
        &database,
        &files,
        &segment(output.clone(), vec![input(naive)]),
    )
    .unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");

    database.record_execute("CREATE TABLE \"events\" (\"at\" TIMESTAMPTZ)", vec![], 0);
    run_segment(
        &database,
        &files,
        &segment(output, vec![input(zoned.clone())]),
    )
    .unwrap();
    let appended = database.appended("events");
    assert_eq!(appended[0].schema().field(0).data_type(), &zoned);
}
