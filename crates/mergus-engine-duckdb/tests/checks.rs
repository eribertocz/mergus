//! MG-REQ-0180: the checks of a segment before it reads or writes a row, with the simulated
//! ports.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use arrow::array::{Array, ArrayRef, Int32Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::{Boundary, EngineError, Segment, Table, Transport, run_segment};
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, Extension, FileSystem, LoadedExtension,
    PortError, SeekRead, SqlValue,
};
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
    let wrong = database(false, "HUGEINT");
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
fn mg_req_0633_output_table_is_apart_from_an_input_of_its_name() {
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
    let create = "CREATE TABLE \"mergus_output\" (\"id\" BIGINT)";
    database.record_execute(create, vec![], 0);
    let copy = "COPY \"mergus_output\" TO ? (FORMAT parquet)";
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

#[test]
fn mg_req_0180_result_widened_exactly_is_accepted() {
    let files = work();
    let database = database(false, "INTEGER");
    let small: ArrayRef = Arc::new(Int32Array::from(vec![Some(1), None]));
    let rows = RecordBatch::try_from_iter([("id", small)]).unwrap();
    database.record_query(SQL, vec![], vec![rows]);
    run_segment(&database, &files, &segment(arrow_output(), vec![])).unwrap();
    let bytes = files.read(Path::new("/work/out.arrow")).unwrap();
    let reader =
        arrow::ipc::reader::FileReader::try_new(std::io::Cursor::new(bytes), None).unwrap();
    let batches: Vec<RecordBatch> = reader.map(|batch| batch.unwrap()).collect();
    let expected = Int64Array::from(vec![Some(1), None]);
    assert_eq!(batches[0].column(0).to_data(), expected.to_data());
}

/// A connection that loads `icu` when it runs `LOAD icu;`, as a macro file could.
struct Loading {
    inner: Box<dyn DatabaseConnection>,
    icu: Arc<AtomicBool>,
}

impl DatabaseConnection for Loading {
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError> {
        if sql == "LOAD icu;" {
            self.icu.store(true, Ordering::SeqCst);
            return Ok(0);
        }
        self.inner.execute(sql, params)
    }

    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError> {
        if sql == LOADED_ICU && self.icu.load(Ordering::SeqCst) {
            let name: ArrayRef = Arc::new(StringArray::from(vec!["icu"]));
            let loaded = RecordBatch::try_from_iter([("extension_name", name)])
                .map_err(|error| PortError::Failed(error.to_string()))?;
            return Ok(vec![loaded]);
        }
        self.inner.query_arrow(sql, params)
    }

    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError> {
        self.inner.append_arrow(table, batch)
    }

    fn attach_ducklake(
        &mut self,
        catalog: &mergus_ports::DuckLakeCatalog,
    ) -> Result<(), PortError> {
        self.inner.attach_ducklake(catalog)
    }

    fn load_extension(&mut self, extension: Extension) -> Result<(), PortError> {
        self.inner.load_extension(extension)
    }

    fn loaded_extensions(&self) -> Vec<LoadedExtension> {
        self.inner.loaded_extensions()
    }
}

struct LoadingDatabase(SimDatabase);

impl Database for LoadingDatabase {
    fn connect(
        &self,
        location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        let inner = self.0.connect(location)?;
        Ok(Box::new(Loading {
            inner,
            icu: Arc::new(AtomicBool::new(false)),
        }))
    }
}

#[test]
fn mg_req_0180_icu_loaded_by_a_macro_fails() {
    let files = work();
    let database = LoadingDatabase(database(false, "BIGINT"));
    let mut loads = segment(arrow_output(), vec![]);
    loads.macros.push("LOAD icu;".to_owned());
    let error = run_segment(&database, &files, &loads).unwrap_err();
    assert_eq!(error.code(), Some("MG-0102"), "{error}");
    assert!(!files.exists(Path::new("/work/out.arrow")).unwrap());
}

/// A file system whose streams fail after `limit` bytes.
struct Failing {
    inner: SimFileSystem,
    limit: usize,
}

struct FailingWriter {
    inner: Box<dyn Write + Send>,
    left: usize,
}

impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.left {
            return Err(io::Error::other("the disk is full"));
        }
        self.left -= bytes.len();
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl FileSystem for Failing {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError> {
        self.inner.read(path)
    }
    fn open(&self, path: &Path) -> Result<Box<dyn SeekRead>, PortError> {
        self.inner.open(path)
    }
    fn create(&self, path: &Path) -> Result<Box<dyn Write + Send>, PortError> {
        let inner = self.inner.create(path)?;
        Ok(Box::new(FailingWriter {
            inner,
            left: self.limit,
        }))
    }
    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError> {
        self.inner.write(path, contents)
    }
    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
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
fn mg_req_0633_failed_boundary_write_leaves_no_file() {
    let files = Failing {
        inner: work(),
        limit: 16,
    };
    let database = database(false, "BIGINT");
    let ids: ArrayRef = Arc::new(Int64Array::from(vec![1, 2, 3]));
    database.record_query(
        SQL,
        vec![],
        vec![RecordBatch::try_from_iter([("id", ids)]).unwrap()],
    );
    let error = run_segment(&database, &files, &segment(arrow_output(), vec![])).unwrap_err();
    assert!(matches!(error, EngineError::Boundary { .. }), "{error}");
    assert!(!files.exists(Path::new("/work/out.arrow")).unwrap());
}
