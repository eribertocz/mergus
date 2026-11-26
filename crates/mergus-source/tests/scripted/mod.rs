//! The scripted `Database` port and the recording clock the source care tests share.

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arrow::array::{ArrayRef, Int64Array};
use arrow::record_batch::RecordBatch;
use mergus_ports::{
    Clock, Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Extension,
    LoadedExtension, MonotonicTime, PortError, SqlValue, Timestamp,
};
use mergus_sim::{ClockFaults, SimClock};
use mergus_source::{DeclaredTable, Projection, ReadOptions};

pub type Log = Arc<Mutex<Vec<(String, Vec<SqlValue>)>>>;
pub type Answer = dyn Fn(&str, &[SqlValue]) -> Result<Vec<RecordBatch>, PortError> + Send + Sync;

/// 2026-11-22T23:15:14Z.
pub const START: i128 = 1_795_389_314_000_000_000;

/// A database whose connections log every statement and answer queries with a script.
#[derive(Clone)]
pub struct Scripted {
    log: Log,
    answer: Arc<Answer>,
}

pub struct ScriptedConnection(Scripted);

impl Scripted {
    pub fn new(
        answer: impl Fn(&str, &[SqlValue]) -> Result<Vec<RecordBatch>, PortError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            log: Log::default(),
            answer: Arc::new(answer),
        }
    }

    pub fn statements(&self) -> Vec<(String, Vec<SqlValue>)> {
        self.log.lock().map(|log| log.clone()).unwrap_or_default()
    }

    fn push(&self, sql: &str, params: &[SqlValue]) {
        if let Ok(mut log) = self.log.lock() {
            log.push((sql.to_owned(), params.to_vec()));
        }
    }
}

impl Database for Scripted {
    fn connect(
        &self,
        _location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        Ok(Box::new(ScriptedConnection(self.clone())))
    }
}

impl DatabaseConnection for ScriptedConnection {
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError> {
        self.0.push(sql, params);
        Ok(0)
    }

    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError> {
        self.0.push(sql, params);
        (self.0.answer)(sql, params)
    }

    fn append_arrow(&mut self, _table: &str, _batch: &RecordBatch) -> Result<(), PortError> {
        Ok(())
    }

    fn attach_ducklake(&mut self, _catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        Ok(())
    }

    fn load_extension(&mut self, _extension: Extension) -> Result<(), PortError> {
        Ok(())
    }

    fn loaded_extensions(&self) -> Vec<LoadedExtension> {
        Vec::new()
    }
}

/// A simulated clock that also records every sleep.
pub struct Recorded {
    inner: Arc<SimClock>,
    sleeps: Mutex<Vec<Duration>>,
}

impl Recorded {
    pub fn new(clock: Arc<SimClock>) -> Self {
        Self {
            inner: clock,
            sleeps: Mutex::new(Vec::new()),
        }
    }

    pub fn sleeps(&self) -> Vec<Duration> {
        self.sleeps
            .lock()
            .map(|sleeps| sleeps.clone())
            .unwrap_or_default()
    }
}

impl Clock for Recorded {
    fn now(&self) -> Timestamp {
        self.inner.now()
    }

    fn monotonic(&self) -> MonotonicTime {
        self.inner.monotonic()
    }

    fn sleep(&self, duration: Duration) {
        if let Ok(mut sleeps) = self.sleeps.lock() {
            sleeps.push(duration);
        }
        self.inner.sleep(duration);
    }
}

pub fn clock_at(start: i128) -> Arc<SimClock> {
    Arc::new(SimClock::new(
        1,
        Timestamp::from_unix_nanos(start),
        ClockFaults::default(),
    ))
}

pub fn orders() -> DeclaredTable {
    DeclaredTable {
        name: "orders".to_owned(),
        key: vec!["id".to_owned()],
        incremental: None,
    }
}

pub fn ids(from: i64, count: i64) -> Vec<RecordBatch> {
    let column: ArrayRef = Arc::new(Int64Array::from((from..from + count).collect::<Vec<_>>()));
    // An error leaves no batch, which every test that reads the rows would see.
    RecordBatch::try_from_iter([("id", column)])
        .into_iter()
        .collect()
}

pub fn chunks_of(rows: u32) -> ReadOptions {
    ReadOptions {
        projection: Projection::All,
        since: None,
        chunk_rows: NonZeroU32::new(rows),
    }
}
