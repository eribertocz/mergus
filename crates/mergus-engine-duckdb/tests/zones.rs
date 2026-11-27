//! MG-REQ-0180: a timestamp keeps its time zone across a segment, with the simulated ports.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, StringArray, TimestampMicrosecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::{AS_OF, Boundary, Segment, Table, Transport, run_segment};
use mergus_ports::{FileSystem, SqlValue};
use mergus_sim::{DatabaseFaults, SimDatabase, SimFileSystem};

const MADRID: &str = "Europe/Madrid";
const LOADED_ICU: &str =
    "SELECT extension_name FROM duckdb_extensions() WHERE loaded AND extension_name = ?";
const SQL: &str = "SELECT at FROM events";

#[cfg(test)]
fn shared(path: &str) -> Boundary {
    Boundary {
        transport: Transport::SharedMemory,
        path: PathBuf::from(path),
    }
}

#[cfg(test)]
fn events(data_type: DataType) -> Table {
    Table {
        name: "events".to_owned(),
        schema: Arc::new(Schema::new(vec![Field::new("at", data_type, true)])),
        boundary: shared("/work/events.arrow"),
    }
}

#[cfg(test)]
fn write_ipc(files: &SimFileSystem, path: &str, batch: &RecordBatch) {
    let mut bytes = Vec::new();
    let mut writer = FileWriter::try_new(&mut bytes, &batch.schema()).unwrap();
    writer.write(batch).unwrap();
    writer.finish().unwrap();
    drop(writer);
    files.write(Path::new(path), &bytes).unwrap();
}

#[cfg(test)]
fn instants(zone: &str) -> RecordBatch {
    let array = TimestampMicrosecondArray::from(vec![Some(1_700_000_000_000_000), None])
        .with_timezone(zone);
    let field = Field::new("at", array.data_type().clone(), true);
    RecordBatch::try_new(Arc::new(Schema::new(vec![field])), vec![Arc::new(array)]).unwrap()
}

#[test]
fn mg_req_0180_timestamps_keep_their_time_zone() {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/work")).unwrap();
    let database = SimDatabase::new(7, DatabaseFaults::default());
    database.record_execute(AS_OF, vec![], 0);
    let icu = vec![SqlValue::Text("icu".to_owned())];
    database.record_query(LOADED_ICU, icu, vec![]);
    let names: ArrayRef = Arc::new(StringArray::from(vec!["at"]));
    let types: ArrayRef = Arc::new(StringArray::from(vec!["TIMESTAMP WITH TIME ZONE"]));
    let description = RecordBatch::try_from_iter([("column_name", names), ("column_type", types)]);
    database.record_query(
        format!("DESCRIBE {SQL}"),
        vec![],
        vec![description.unwrap()],
    );
    write_ipc(&files, "/work/events.arrow", &instants(MADRID));
    let zoned = DataType::Timestamp(TimeUnit::Microsecond, Some(MADRID.into()));
    let naive = DataType::Timestamp(TimeUnit::Microsecond, None);
    let mut output = events(zoned.clone());
    output.name = "out".to_owned();
    output.boundary = shared("/work/out.arrow");
    let mut segment = Segment {
        sql: SQL.to_owned(),
        params: vec![],
        macros: vec![],
        inputs: vec![events(naive)],
        output,
    };
    let naive_table = "CREATE TABLE \"events\" (\"at\" TIMESTAMP)";
    database.record_execute(naive_table, vec![], 0);
    let error = run_segment(&database, &files, &segment).unwrap_err();
    assert_eq!(error.code(), Some("MG-0101"), "{error}");

    // DuckDB keeps the instant of a TIMESTAMPTZ and gives it back in UTC.
    let create = "CREATE TABLE \"events\" (\"at\" TIMESTAMP WITH TIME ZONE)";
    database.record_execute(create, vec![], 0);
    database.record_query(SQL, vec![], vec![instants("UTC")]);
    segment.inputs = vec![events(zoned.clone())];
    run_segment(&database, &files, &segment).unwrap();
    assert_eq!(database.appended("events"), vec![instants(MADRID)]);
    let bytes = files.read(Path::new("/work/out.arrow")).unwrap();
    let reader = FileReader::try_new(Cursor::new(bytes), None).unwrap();
    let schema = reader.schema();
    let batches: Vec<RecordBatch> = reader.map(|batch| batch.unwrap()).collect();
    assert_eq!(schema.field(0).data_type(), &zoned);
    assert_eq!(batches, vec![instants(MADRID)]);
}
