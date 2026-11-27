//! MG-REQ-0643: every DuckDB session that runs a segment loads the project's macros first.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arrow::array::{ArrayRef, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::{
    Boundary, EngineError, Segment, Table, Transport, run_segment, table_hash,
};
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

/// A database that records the location of each connection.
struct Counted {
    inner: SimDatabase,
    locations: Mutex<Vec<DatabaseLocation>>,
}

impl Database for Counted {
    fn connect(
        &self,
        location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        if let Ok(mut locations) = self.locations.lock() {
            locations.push(location.clone());
        }
        self.inner.connect(location)
    }
}

#[test]
fn mg_req_0643_macros_loaded_in_each_session() {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/zone/work")).unwrap();
    let database = Counted {
        inner: database(),
        locations: Mutex::new(Vec::new()),
    };
    run_segment(&database, &files, &segment()).unwrap();
    run_segment(&database, &files, &segment()).unwrap();
    // Each segment runs in its own in-memory session; its Parquet output is gathered in a
    // second session, on a scratch file, which runs no step SQL.
    let scratch = DatabaseLocation::File(PathBuf::from("/zone/work/a.parquet.duckdb"));
    let expected = [DatabaseLocation::InMemory, scratch];
    assert_eq!(
        *database.locations.lock().unwrap(),
        [expected.clone(), expected].concat()
    );
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

#[test]
fn mg_req_0633_parquet_output_streams_through_a_scratch_file() {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/zone/work")).unwrap();
    // A scratch file left by an earlier run is replaced, and removed at the end.
    let scratch = Path::new("/zone/work/a.parquet.duckdb");
    files.write(scratch, b"stale").unwrap();
    files
        .write(Path::new("/zone/work/a.parquet.duckdb.wal"), b"stale")
        .unwrap();
    let spill = Path::new("/zone/work/a.parquet.duckdb.tmp");
    files.create_dir_all(spill).unwrap();
    files.write(&spill.join("spill.block"), b"stale").unwrap();
    let database = database();
    let id = |values: Vec<i64>| {
        let ids: ArrayRef = Arc::new(Int64Array::from(values));
        RecordBatch::try_from_iter([("id", ids)]).unwrap()
    };
    let parts = vec![id(vec![1]), id(vec![2, 3]), id(vec![4])];
    database.record_query(SQL, vec![], parts.clone());
    run_segment(&database, &files, &segment()).unwrap();
    let appended = database.appended("mergus_output");
    assert_eq!(appended.len(), 3);
    assert_eq!(table_hash(&appended).unwrap(), table_hash(&parts).unwrap());
    assert!(!files.exists(scratch).unwrap());
    assert!(
        !files
            .exists(Path::new("/zone/work/a.parquet.duckdb.wal"))
            .unwrap()
    );
    assert!(!files.exists(spill).unwrap());
}

/// A file system that cannot remove one file, as when another process holds it locked.
struct Locked {
    inner: SimFileSystem,
    locked: PathBuf,
}

impl FileSystem for Locked {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError> {
        self.inner.read(path)
    }
    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError> {
        self.inner.write(path, contents)
    }
    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        if path == self.locked {
            return Err(PortError::Failed(format!("`{}` is locked", path.display())));
        }
        self.inner.remove_file(path)
    }
    fn create_dir_all(&self, path: &Path) -> Result<(), PortError> {
        self.inner.create_dir_all(path)
    }
    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, PortError> {
        self.inner.list_dir(path)
    }
    fn exists(&self, path: &Path) -> Result<bool, PortError> {
        self.inner.exists(path)
    }
}

#[test]
fn mg_req_0633_stale_scratch_that_cannot_be_removed_fails_first() {
    let scratch = PathBuf::from("/zone/work/a.parquet.duckdb");
    let files = Locked {
        inner: SimFileSystem::new(),
        locked: scratch.clone(),
    };
    files.create_dir_all(Path::new("/zone/work")).unwrap();
    files.write(&scratch, b"stale").unwrap();
    let database = Counted {
        inner: database(),
        locations: Mutex::new(Vec::new()),
    };
    let error = run_segment(&database, &files, &segment()).unwrap_err();
    // The engine names the scratch file after the output file, next to it.
    let output = segment().output.boundary.path;
    let shown = output
        .with_file_name("a.parquet.duckdb")
        .display()
        .to_string();
    assert!(
        matches!(&error, EngineError::Scratch { path, .. } if *path == shown),
        "{error}"
    );
    // The step ran in its own session; the scratch session never opened.
    assert_eq!(
        *database.locations.lock().unwrap(),
        [DatabaseLocation::InMemory]
    );
}
