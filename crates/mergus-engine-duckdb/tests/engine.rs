//! The behavior of a segment, with the simulated `Database` and `FileSystem` ports.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, Decimal128Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::{
    AS_OF, Boundary, EngineError, MEMORY_BUDGET, Segment, Table, Transport, run_segment, table_hash,
};
use mergus_ports::{FileSystem, SqlValue};
use mergus_sim::{DatabaseFaults, SimDatabase, SimFileSystem};

const SQL: &str = "SELECT id, name FROM orders WHERE id > ?";
const MACROS: [&str; 2] = [
    "CREATE MACRO safe_div(a, b) AS CASE WHEN b = 0 THEN NULL ELSE a / b END;",
    "CREATE MACRO clean(t) AS TABLE SELECT * FROM query_table(t) WHERE status <> 'VOID';",
];
const CREATE_OUTPUT: &str = "CREATE TABLE \"mergus_output\" (\"id\" BIGINT, \"name\" VARCHAR)";
const CREATE_INPUT: &str = "CREATE TABLE \"orders\" (\"id\" BIGINT, \"name\" VARCHAR)";
const LOADED_ICU: &str =
    "SELECT extension_name FROM duckdb_extensions() WHERE loaded AND extension_name = ?";
const COPY: &str = "COPY \"mergus_output\" TO ? (FORMAT parquet)";
const SET_INPUT: &str = "SET VARIABLE mergus_input_0 = ?";
const DESCRIBE_INPUT: &str = "DESCRIBE SELECT * FROM read_parquet(getvariable('mergus_input_0'))";
const VIEW_INPUT: &str = "CREATE VIEW \"orders\" AS SELECT \"id\", \"name\" FROM \
                          read_parquet(getvariable('mergus_input_0'))";

#[cfg(test)]
fn rows(ids: &[i64], names: &[&str]) -> RecordBatch {
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(ids.to_vec())),
        Arc::new(StringArray::from(names.to_vec())),
    ];
    RecordBatch::try_new(rows_schema(), columns).unwrap()
}

#[cfg(test)]
fn rows_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, true),
        Field::new("name", DataType::Utf8, true),
    ]))
}

#[cfg(test)]
fn boundary(transport: Transport, path: &str) -> Boundary {
    Boundary {
        transport,
        path: PathBuf::from(path),
    }
}

#[cfg(test)]
fn table(name: &str, schema: SchemaRef, boundary: Boundary) -> Table {
    Table {
        name: name.to_owned(),
        schema,
        boundary,
    }
}

#[cfg(test)]
fn segment(output: Boundary, inputs: Vec<Table>) -> Segment {
    Segment {
        sql: SQL.to_owned(),
        params: vec![SqlValue::Integer(1)],
        macros: MACROS.iter().map(|text| (*text).to_owned()).collect(),
        catalogs: Vec::new(),
        snapshot: None,
        inputs,
        output: table("orders_out", rows_schema(), output),
    }
}

/// A database that answers the session settings, the macros and the segment's statements.
#[cfg(test)]
fn database() -> SimDatabase {
    let database = SimDatabase::new(7, DatabaseFaults::default());
    database.record_execute(AS_OF, vec![], 0);
    database.record_query(LOADED_ICU, vec![SqlValue::Text("icu".to_owned())], vec![]);
    for text in MACROS {
        database.record_execute(text, vec![], 0);
    }
    let columns = described(&[("id", "BIGINT"), ("name", "VARCHAR")]);
    database.record_query(
        format!("DESCRIBE {SQL}"),
        vec![SqlValue::Integer(1)],
        columns,
    );
    let params = vec![SqlValue::Integer(1)];
    database.record_query(SQL, params, vec![rows(&[2, 3], &["b", "c"])]);
    database.record_execute(CREATE_OUTPUT, vec![], 0);
    database
}

#[cfg(test)]
fn work() -> SimFileSystem {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/work")).unwrap();
    files
}

#[cfg(test)]
fn write_ipc(files: &SimFileSystem, path: &str, batches: &[RecordBatch]) {
    let mut bytes = Vec::new();
    let mut writer = FileWriter::try_new(&mut bytes, &batches[0].schema()).unwrap();
    for batch in batches {
        writer.write(batch).unwrap();
    }
    writer.finish().unwrap();
    drop(writer);
    files.write(Path::new(path), &bytes).unwrap();
}

#[cfg(test)]
fn read_ipc(files: &SimFileSystem, path: &str) -> (SchemaRef, Vec<RecordBatch>) {
    let bytes = files.read(Path::new(path)).unwrap();
    let reader = FileReader::try_new(Cursor::new(bytes), None).unwrap();
    let schema = reader.schema();
    (schema, reader.map(|batch| batch.unwrap()).collect())
}

#[cfg(test)]
fn executed(database: &SimDatabase) -> Vec<String> {
    let statements = database.executed().into_iter();
    statements.map(|(sql, _)| sql).collect()
}

/// The answer of `DESCRIBE` for a Parquet file with these columns and DuckDB types.
#[cfg(test)]
fn described(columns: &[(&str, &str)]) -> Vec<RecordBatch> {
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
fn parquet_input(database: &SimDatabase, columns: &[(&str, &str)]) -> Table {
    let path = SqlValue::Text("/zone/work/orders.parquet".to_owned());
    database.record_execute(SET_INPUT, vec![path], 0);
    database.record_query(DESCRIBE_INPUT, vec![], described(columns));
    database.record_execute(VIEW_INPUT, vec![], 0);
    let file = boundary(Transport::Parquet, "/zone/work/orders.parquet");
    table("orders", rows_schema(), file)
}

#[test]
fn mg_req_0633_transport_falls_back_to_parquet() {
    assert_eq!(Transport::for_size(MEMORY_BUDGET), Transport::SharedMemory);
    assert_eq!(Transport::for_size(MEMORY_BUDGET + 1), Transport::Parquet);

    let files = work();
    let database = database();
    let arrow = boundary(Transport::SharedMemory, "/work/orders.arrow");
    run_segment(&database, &files, &segment(arrow, vec![])).unwrap();
    let (_, read) = read_ipc(&files, "/work/orders.arrow");
    assert_eq!(read, vec![rows(&[2, 3], &["b", "c"])]);
    assert!(!executed(&database).iter().any(|sql| sql == COPY));

    let path = SqlValue::Text("/zone/work/orders.parquet".to_owned());
    database.record_execute(COPY, vec![path.clone()], 2);
    let parquet = boundary(Transport::Parquet, "/zone/work/orders.parquet");
    run_segment(&database, &files, &segment(parquet, vec![])).unwrap();
    assert!(database.executed().contains(&(COPY.to_owned(), vec![path])));
    assert_eq!(
        database.appended("mergus_output"),
        vec![rows(&[2, 3], &["b", "c"])]
    );
    assert!(
        !files
            .exists(Path::new("/zone/work/orders.parquet"))
            .unwrap()
    );
}

#[test]
fn mg_req_0633_shared_memory_streams_batch_by_batch() {
    let files = work();
    let database = database();
    let parts = vec![
        rows(&[1], &["a"]),
        rows(&[2, 3], &["b", "c"]),
        rows(&[4], &["d"]),
    ];
    let mut bytes = Vec::new();
    let mut writer = FileWriter::try_new(&mut bytes, &rows_schema()).unwrap();
    for part in &parts {
        writer.write(part).unwrap();
    }
    writer.finish().unwrap();
    drop(writer);
    files
        .write(Path::new("/work/orders.arrow"), &bytes)
        .unwrap();
    database.record_execute(CREATE_INPUT, vec![], 0);
    database.record_query(SQL, vec![SqlValue::Integer(1)], parts.clone());
    let input = table(
        "orders",
        rows_schema(),
        boundary(Transport::SharedMemory, "/work/orders.arrow"),
    );
    let output = boundary(Transport::SharedMemory, "/work/out.arrow");
    run_segment(&database, &files, &segment(output, vec![input])).unwrap();
    assert_eq!(database.appended("orders"), parts);
    let (_, written) = read_ipc(&files, "/work/out.arrow");
    assert_eq!(written.len(), 3);
    assert_eq!(table_hash(&written).unwrap(), table_hash(&parts).unwrap());
}

#[test]
fn mg_req_0633_parquet_input_is_read_lazily() {
    let files = work();
    let database = database();
    let input = parquet_input(&database, &[("id", "BIGINT"), ("name", "VARCHAR")]);
    let output = boundary(Transport::SharedMemory, "/work/out.arrow");
    run_segment(&database, &files, &segment(output, vec![input])).unwrap();
    let path = SqlValue::Text("/zone/work/orders.parquet".to_owned());
    assert!(
        database
            .executed()
            .contains(&(SET_INPUT.to_owned(), vec![path]))
    );
    assert!(executed(&database).contains(&VIEW_INPUT.to_owned()));
    assert!(database.appended("orders").is_empty());
}

#[test]
fn mg_req_0180_parquet_input_of_another_type_fails() {
    let files = work();
    let database = database();
    let input = parquet_input(&database, &[("id", "INTEGER"), ("name", "VARCHAR")]);
    let output = boundary(Transport::SharedMemory, "/work/out.arrow");
    let error = run_segment(&database, &files, &segment(output, vec![input])).unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");
    assert!(
        matches!(&error, EngineError::Unrepresentable { table, column, .. }
        if table == "orders" && column == "id")
    );
    assert!(!executed(&database).contains(&VIEW_INPUT.to_owned()));
    assert!(!files.exists(Path::new("/work/out.arrow")).unwrap());

    let missing = parquet_input(&database, &[("id", "BIGINT")]);
    let output = boundary(Transport::SharedMemory, "/work/out.arrow");
    let error = run_segment(&database, &files, &segment(output, vec![missing])).unwrap_err();
    assert!(matches!(&error, EngineError::Unrepresentable { column, .. } if column == "name"));
}

#[test]
fn mg_req_0633_empty_output_keeps_its_columns() {
    let files = work();
    let database = database();
    database.record_query(SQL, vec![SqlValue::Integer(1)], vec![]);
    let arrow = boundary(Transport::SharedMemory, "/work/empty.arrow");
    run_segment(&database, &files, &segment(arrow, vec![])).unwrap();
    let (schema, batches) = read_ipc(&files, "/work/empty.arrow");
    assert_eq!(schema, rows_schema());
    assert!(batches.iter().all(|batch| batch.num_rows() == 0));

    let path = SqlValue::Text("/zone/work/empty.parquet".to_owned());
    database.record_execute(COPY, vec![path.clone()], 0);
    let parquet = boundary(Transport::Parquet, "/zone/work/empty.parquet");
    run_segment(&database, &files, &segment(parquet, vec![])).unwrap();
    let statements = executed(&database);
    assert!(statements.contains(&CREATE_OUTPUT.to_owned()));
    assert!(database.executed().contains(&(COPY.to_owned(), vec![path])));
}

/// Writes `values` as a column `amount` of type `DECIMAL(precision, scale)` and returns the
/// input `amounts` that declares the column as `to`.
#[cfg(test)]
fn decimal_input(
    files: &SimFileSystem,
    values: Vec<Option<i128>>,
    (precision, scale): (u8, i8),
    to: DataType,
) -> Table {
    let array = Decimal128Array::from(values)
        .with_precision_and_scale(precision, scale)
        .unwrap();
    let from = DataType::Decimal128(precision, scale);
    let schema = Arc::new(Schema::new(vec![Field::new("amount", from, true)]));
    let batch = RecordBatch::try_new(schema, vec![Arc::new(array)]).unwrap();
    write_ipc(files, "/work/amounts.arrow", &[batch]);
    let declared = Arc::new(Schema::new(vec![Field::new("amount", to, true)]));
    let file = boundary(Transport::SharedMemory, "/work/amounts.arrow");
    table("amounts", declared, file)
}

#[cfg(test)]
fn run_with(
    database: &SimDatabase,
    files: &SimFileSystem,
    input: Table,
) -> Result<(), EngineError> {
    let output = boundary(Transport::SharedMemory, "/work/out.arrow");
    run_segment(database, files, &segment(output, vec![input]))
}

/// Asserts that `result` is `MG-0101` on the column `amount` of `amounts`, with nothing
/// appended.
#[cfg(test)]
fn assert_unrepresentable(result: Result<(), EngineError>, database: &SimDatabase) {
    let error = result.unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");
    assert!(
        matches!(&error, EngineError::Unrepresentable { table, column, .. }
        if table == "amounts" && column == "amount")
    );
    assert!(database.appended("amounts").is_empty());
}

#[test]
fn mg_req_0180_unrepresentable_value_fails() {
    let files = work();
    let database = database();
    for create in [
        "CREATE TABLE \"amounts\" (\"amount\" DECIMAL(8,2))",
        "CREATE TABLE \"amounts\" (\"amount\" DECIMAL(10,1))",
        "CREATE TABLE \"amounts\" (\"amount\" BIGINT)",
    ] {
        database.record_execute(create, vec![], 0);
    }

    // A DECIMAL overflow: 1234567890.12 does not fit DECIMAL(8, 2).
    let values = vec![Some(1), Some(123_456_789_012)];
    let input = decimal_input(&files, values, (12, 2), DataType::Decimal128(8, 2));
    assert_unrepresentable(run_with(&database, &files, input), &database);

    // A lost digit: 1.25 is not 1.2 or 1.3.
    let input = decimal_input(
        &files,
        vec![Some(125)],
        (10, 2),
        DataType::Decimal128(10, 1),
    );
    assert_unrepresentable(run_with(&database, &files, input), &database);

    // A HUGEINT beyond BIGINT: DuckDB gives HUGEINT to Arrow as DECIMAL(38, 0).
    let input = decimal_input(&files, vec![Some(1_i128 << 70)], (38, 0), DataType::Int64);
    assert_unrepresentable(run_with(&database, &files, input), &database);
    assert!(!files.exists(Path::new("/work/out.arrow")).unwrap());
}

#[test]
fn mg_req_0180_exact_values_are_kept() {
    let files = work();
    let database = database();
    let create = "CREATE TABLE \"amounts\" (\"amount\" DECIMAL(14,2))";
    database.record_execute(create, vec![], 0);
    let values = vec![Some(123_456_789_012), None];
    let input = decimal_input(&files, values, (12, 2), DataType::Decimal128(14, 2));
    run_with(&database, &files, input).unwrap();
    let appended = database.appended("amounts");
    let expected = Decimal128Array::from(vec![Some(123_456_789_012), None])
        .with_precision_and_scale(14, 2)
        .unwrap();
    assert_eq!(appended.len(), 1);
    assert_eq!(appended[0].column(0).to_data(), expected.to_data());

    database.record_execute("CREATE TABLE \"amounts\" (\"amount\" BIGINT)", vec![], 0);
    let input = decimal_input(&files, vec![Some(42), None], (38, 0), DataType::Int64);
    run_with(&database, &files, input).unwrap();
    let last = database.appended("amounts").pop().unwrap();
    let expected = Int64Array::from(vec![Some(42), None]);
    assert_eq!(last.column(0).to_data(), expected.to_data());
}
