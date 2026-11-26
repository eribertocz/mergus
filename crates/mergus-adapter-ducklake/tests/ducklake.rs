//! MG-REQ-0286 and MG-REQ-0288 on the simulated `Database`: it answers each statement with the
//! result recorded for it, and a wrapper logs every call the adapter makes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use arrow::array::{ArrayRef, AsArray, Int64Array, RecordBatch, RecordBatchIterator};
use arrow::datatypes::{DataType, Field, Int64Type, Schema};
use mergus_adapter::{AdapterError, BatchStream, Retention, StorageAdapter, VersionRef, WriteMode};
use mergus_adapter_ducklake::DuckLakeAdapter;
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Extension, LoadedExtension,
    PortError, SqlValue, Timestamp,
};
use mergus_sim::{ClockFaults, DatabaseFaults, SimClock, SimDatabase};

const SNAPSHOT_EXISTS: &str =
    "SELECT count(*) AS found FROM ducklake_snapshots(?) WHERE snapshot_id = ?";
const READ_ORDERS: &str = "SELECT * FROM \"lake\".\"orders\" AT (VERSION => ?)";
const CREATE_STAGE: &str = "CREATE TABLE \"mergus_stage_0\" (\"id\" BIGINT)";
const LATEST: &str = "SELECT max(snapshot_id) AS snapshot_id FROM ducklake_snapshots(?)";
const SNAPSHOTS: &str = "SELECT snapshot_id, epoch_us(snapshot_time) AS micros \
                         FROM ducklake_snapshots(?) ORDER BY snapshot_id";
const EXPIRE: &str = "CALL ducklake_expire_snapshots(?, versions => [?])";
const CLEANUP: &str = "CALL ducklake_cleanup_old_files(?, cleanup_all => true)";

/// The statements `commit` runs to publish the rows staged for `orders` with append.
const COMMIT_APPEND: [&str; 5] = [
    "BEGIN TRANSACTION",
    "CREATE TABLE IF NOT EXISTS \"lake\".\"orders\" AS SELECT * FROM \"mergus_stage_0\" LIMIT 0",
    "INSERT INTO \"lake\".\"orders\" BY NAME SELECT * FROM \"mergus_stage_0\"",
    "COMMIT",
    "DROP TABLE \"mergus_stage_0\"",
];

/// A `Database` that logs the SQL of every call of its connections, then delegates.
#[derive(Clone)]
struct Logged {
    inner: SimDatabase,
    log: Arc<Mutex<Vec<String>>>,
}

#[cfg(test)]
impl Logged {
    fn new() -> Self {
        Self {
            inner: SimDatabase::new(1, DatabaseFaults::default()),
            log: Arc::default(),
        }
    }

    fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.lock().unwrap())
    }
}

#[cfg(test)]
impl Database for Logged {
    fn connect(
        &self,
        location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        Ok(Box::new(LoggedConnection {
            inner: self.inner.connect(location)?,
            log: Arc::clone(&self.log),
        }))
    }
}

struct LoggedConnection {
    inner: Box<dyn DatabaseConnection>,
    log: Arc<Mutex<Vec<String>>>,
}

#[cfg(test)]
impl LoggedConnection {
    fn push(&self, entry: String) {
        self.log.lock().unwrap().push(entry);
    }
}

#[cfg(test)]
impl DatabaseConnection for LoggedConnection {
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError> {
        self.push(sql.to_owned());
        self.inner.execute(sql, params)
    }

    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError> {
        self.push(sql.to_owned());
        self.inner.query_arrow(sql, params)
    }

    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError> {
        self.push(format!("append to {table}"));
        self.inner.append_arrow(table, batch)
    }

    fn attach_ducklake(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.push(format!("catalog {} attached", catalog.alias));
        self.inner.attach_ducklake(catalog)
    }

    fn load_extension(&mut self, extension: Extension) -> Result<(), PortError> {
        self.push(format!("extension {extension:?} loaded"));
        self.inner.load_extension(extension)
    }

    fn loaded_extensions(&self) -> Vec<LoadedExtension> {
        self.inner.loaded_extensions()
    }
}

#[cfg(test)]
fn catalog() -> DuckLakeCatalog {
    DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: "metadata.ducklake".to_owned(),
        data_path: "data/".to_owned(),
    }
}

#[cfg(test)]
fn clock(seconds: i128) -> Arc<SimClock> {
    let start = Timestamp::from_unix_nanos(seconds * 1_000_000_000);
    Arc::new(SimClock::new(1, start, ClockFaults::default()))
}

#[cfg(test)]
fn text(value: &str) -> SqlValue {
    SqlValue::Text(value.to_owned())
}

#[cfg(test)]
/// One batch of the columns `columns`, each a `BIGINT` column of `values`.
fn int_batch(columns: &[(&str, Vec<i64>)]) -> RecordBatch {
    let pairs = columns.iter().map(|(name, values)| {
        let column: ArrayRef = Arc::new(Int64Array::from(values.clone()));
        (*name, column)
    });
    RecordBatch::try_from_iter(pairs).unwrap()
}

#[cfg(test)]
fn orders(ids: Vec<i64>) -> BatchStream {
    let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, true)]));
    let batch = int_batch(&[("id", ids)]);
    Box::new(RecordBatchIterator::new([Ok(batch)], schema))
}

#[cfg(test)]
fn ids(stream: BatchStream) -> Vec<i64> {
    stream
        .flat_map(|batch| {
            let batch = batch.unwrap();
            let column = batch.column(0).as_primitive::<Int64Type>().clone();
            column.values().to_vec()
        })
        .collect()
}

#[cfg(test)]
/// Records that snapshot `id` exists and that `orders` holds `rows` at it.
fn record_published(database: &SimDatabase, id: i64, rows: Vec<i64>) {
    let exists = vec![text("lake"), SqlValue::Integer(id)];
    database.record_query(
        SNAPSHOT_EXISTS,
        exists,
        vec![int_batch(&[("found", vec![1])])],
    );
    let at = vec![SqlValue::Integer(id)];
    database.record_query(READ_ORDERS, at, vec![int_batch(&[("id", rows)])]);
}

#[test]
fn mg_req_0286_write_invisible_before_commit() {
    let database = Logged::new();
    record_published(&database.inner, 1, vec![1, 2]);
    database.inner.record_execute(CREATE_STAGE, Vec::new(), 0);
    let mut adapter = DuckLakeAdapter::open(&database, &catalog(), clock(0)).unwrap();
    let published = VersionRef::new("1");
    database.take_log();

    // `write` stages the rows outside the catalog: no statement touches `lake`.
    adapter
        .write("orders", WriteMode::Append, orders(vec![3, 4]))
        .unwrap();
    let log = database.take_log();
    assert_eq!(log, [CREATE_STAGE, "append to mergus_stage_0"]);
    assert_eq!(
        database.inner.appended("mergus_stage_0"),
        [int_batch(&[("id", vec![3, 4])])]
    );
    // Readers of the published version still see only its rows.
    assert_eq!(ids(adapter.read_at("orders", &published).unwrap()), [1, 2]);

    // `commit` publishes the staged rows in one transaction and returns the new snapshot.
    for statement in COMMIT_APPEND {
        database.inner.record_execute(statement, Vec::new(), 0);
    }
    let latest = vec![int_batch(&[("snapshot_id", vec![2])])];
    database
        .inner
        .record_query(LATEST, vec![text("lake")], latest);
    database.take_log();
    assert_eq!(adapter.commit().unwrap(), VersionRef::new("2"));
    let mut expected: Vec<&str> = COMMIT_APPEND.to_vec();
    expected.push(LATEST);
    assert_eq!(database.take_log(), expected);
}

#[test]
fn mg_req_0286_failed_commit_publishes_nothing() {
    let database = Logged::new();
    database.inner.record_execute(CREATE_STAGE, Vec::new(), 0);
    let mut adapter = DuckLakeAdapter::open(&database, &catalog(), clock(0)).unwrap();
    adapter
        .write("orders", WriteMode::Replace, orders(vec![5]))
        .unwrap();
    // The transaction starts, then the catalog refuses the table: the adapter rolls back.
    database
        .inner
        .record_execute(COMMIT_APPEND[0], Vec::new(), 0);
    database.inner.record_execute("ROLLBACK", Vec::new(), 0);
    database.take_log();
    assert!(matches!(adapter.commit(), Err(AdapterError::Storage(_))));
    let log = database.take_log();
    assert_eq!(log.first().map(String::as_str), Some(COMMIT_APPEND[0]));
    assert_eq!(log.last().map(String::as_str), Some("ROLLBACK"));
    assert!(
        !log.iter().any(|statement| statement == "COMMIT"),
        "{log:?}"
    );
}

#[test]
fn mg_req_0288_expire_keeps_referenced_versions() {
    let database = Logged::new();
    let mut adapter = DuckLakeAdapter::open(&database, &catalog(), clock(10_000)).unwrap();
    // Snapshots 1 to 5, taken at these seconds; the clock reads 10 000 s.
    let taken = [1_000, 2_000, 3_000, 9_000, 9_500];
    let micros: Vec<i64> = taken.iter().map(|seconds| seconds * 1_000_000).collect();
    let rows = int_batch(&[("snapshot_id", vec![1, 2, 3, 4, 5]), ("micros", micros)]);
    database
        .inner
        .record_query(SNAPSHOTS, vec![text("lake")], vec![rows]);
    for id in [1, 3, 4] {
        let params = vec![text("lake"), SqlValue::Integer(id)];
        database.inner.record_query(EXPIRE, params, Vec::new());
    }
    database
        .inner
        .record_query(CLEANUP, vec![text("lake")], Vec::new());
    database.take_log();

    // Snapshot 2 is live; 5 is younger than the window and the published one. Snapshot 4 is
    // exactly as old as the window, so it is outside it.
    let retention = Retention {
        live: vec![VersionRef::new("2")],
        rollback_window: Duration::from_secs(1_000),
    };
    adapter.expire(&retention).unwrap();
    assert_eq!(
        database.take_log(),
        [SNAPSHOTS, EXPIRE, EXPIRE, EXPIRE, CLEANUP]
    );

    // With every old snapshot live or inside the window, nothing is expired.
    let retention = Retention {
        live: vec![
            VersionRef::new("1"),
            VersionRef::new("2"),
            VersionRef::new("3"),
        ],
        rollback_window: Duration::from_secs(1_500),
    };
    adapter.expire(&retention).unwrap();
    assert_eq!(database.take_log(), [SNAPSHOTS]);
}

#[test]
fn read_at_an_unknown_version_fails_typed() {
    let database = Logged::new();
    let exists = vec![text("lake"), SqlValue::Integer(9)];
    let none = vec![int_batch(&[("found", vec![0])])];
    database.inner.record_query(SNAPSHOT_EXISTS, exists, none);
    let adapter = DuckLakeAdapter::open(&database, &catalog(), clock(0)).unwrap();
    for version in ["9", "not a snapshot"] {
        let error = adapter
            .read_at("orders", &VersionRef::new(version))
            .err()
            .unwrap();
        let AdapterError::VersionUnavailable { table, version: at } = error else {
            panic!("not a `VersionUnavailable` error: {error}");
        };
        assert_eq!((table.as_str(), at.as_str()), ("orders", version));
    }
}

#[test]
fn open_attaches_the_catalog_and_declares_its_capabilities() {
    let database = Logged::new();
    let adapter = DuckLakeAdapter::open(&database, &catalog(), clock(0)).unwrap();
    assert_eq!(database.inner.attached(), [catalog()]);
    let capabilities = adapter.capabilities();
    assert_eq!(
        capabilities.write_modes,
        [WriteMode::Append, WriteMode::Replace]
    );
    assert!(capabilities.multi_table_transactions);
    assert!(capabilities.exact_types.contains(&DataType::Int64));
}
