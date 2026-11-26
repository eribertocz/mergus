//! MG-REQ-0426, MG-REQ-0427, MG-REQ-0428 and MG-REQ-0430: source care, checked on the
//! statements sent to a scripted `Database` port and on a simulated clock.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arrow::array::{ArrayRef, Int64Array};
use arrow::record_batch::RecordBatch;
use mergus_ports::{
    Clock, Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Extension,
    LoadedExtension, MonotonicTime, PortError, SqlValue, Timestamp,
};
use mergus_project::{ConfigFile, Source};
use mergus_sim::{ClockFaults, SimClock};
use mergus_source::{
    Brake, Budget, DeclaredTable, Load, LoadKind, Projection, ReadOptions, SourceCare, Window,
    attach_postgres, read_with_brake, wait_for_window,
};
use serde_json::{Value, json};

type Log = Arc<Mutex<Vec<(String, Vec<SqlValue>)>>>;
type Answer = dyn Fn(&str, &[SqlValue]) -> Result<Vec<RecordBatch>, PortError> + Send + Sync;

/// 2026-11-22T23:15:14Z.
const START: i128 = 1_795_389_314_000_000_000;

/// A database whose connections log every statement and answer queries with a script.
#[derive(Clone)]
struct Scripted {
    log: Log,
    answer: Arc<Answer>,
}

struct ScriptedConnection(Scripted);

impl Scripted {
    fn new(
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

    fn statements(&self) -> Vec<(String, Vec<SqlValue>)> {
        self.log.lock().map(|log| log.clone()).unwrap_or_default()
    }

    fn push(&self, sql: &str, params: &[SqlValue]) {
        if let Ok(mut log) = self.log.lock() {
            log.push((sql.to_owned(), params.to_vec()));
        }
    }

    /// The parameters of every statement whose text starts with `prefix`.
    fn params_of(&self, prefix: &str) -> Vec<Vec<SqlValue>> {
        self.statements()
            .into_iter()
            .filter(|(sql, _)| sql.starts_with(prefix))
            .map(|(_, params)| params)
            .collect()
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
struct Recorded {
    inner: Arc<SimClock>,
    sleeps: Mutex<Vec<Duration>>,
}

impl Recorded {
    fn new(clock: Arc<SimClock>) -> Self {
        Self {
            inner: clock,
            sleeps: Mutex::new(Vec::new()),
        }
    }

    fn sleeps(&self) -> Vec<Duration> {
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

fn clock_at(start: i128) -> Arc<SimClock> {
    Arc::new(SimClock::new(
        1,
        Timestamp::from_unix_nanos(start),
        ClockFaults::default(),
    ))
}

fn source(options: &Value) -> Source {
    let mut value = json!({
        "kind": "database",
        "engine": "postgresql",
        "connection": "secret:erp",
        "tables": { "orders": { "key": "id" } }
    });
    for (key, option) in options.as_object().into_iter().flatten() {
        value[key] = option.clone();
    }
    Source {
        name: "erp".to_owned(),
        file: ConfigFile {
            path: "sources/erp.toml".to_owned(),
            canonical: value.to_string(),
            value,
        },
    }
}

fn orders() -> DeclaredTable {
    DeclaredTable {
        name: "orders".to_owned(),
        key: vec!["id".to_owned()],
        incremental: None,
    }
}

fn ids(from: i64, count: i64) -> Vec<RecordBatch> {
    let column: ArrayRef = Arc::new(Int64Array::from((from..from + count).collect::<Vec<_>>()));
    // An error leaves no batch, which every test that reads the rows would see.
    RecordBatch::try_from_iter([("id", column)])
        .into_iter()
        .collect()
}

fn chunks_of(rows: u32) -> ReadOptions {
    ReadOptions {
        projection: Projection::All,
        since: None,
        chunk_rows: NonZeroU32::new(rows),
    }
}

/// A PostgreSQL server that answers `version()` and serves `orders` in chunks.
fn server(total: i64) -> Scripted {
    Scripted::new(move |sql, params| {
        if sql.starts_with("SELECT * FROM postgres_query") {
            let column: ArrayRef =
                Arc::new(arrow::array::StringArray::from(vec!["PostgreSQL 17.2"]));
            return Ok(RecordBatch::try_from_iter([("version", column)])
                .into_iter()
                .collect());
        }
        let limit = match params.last() {
            Some(SqlValue::Integer(limit)) => *limit,
            _ => total,
        };
        let after = match params.first() {
            Some(SqlValue::Integer(after)) if params.len() == 2 => *after,
            _ => 0,
        };
        Ok(ids(after + 1, limit.min(total - after).max(0)))
    })
}

#[test]
fn mg_req_0426_queries_go_to_replica() {
    let secrets = BTreeMap::from([
        ("secret:erp", "postgresql://primary/erp"),
        ("secret:erp_replica", "postgresql://replica/erp"),
    ]);
    for (options, expected) in [
        (
            json!({ "replica": "secret:erp_replica" }),
            "postgresql://replica/erp",
        ),
        (json!({}), "postgresql://primary/erp"),
    ] {
        let care = SourceCare::of(&source(&options), "prod");
        let reference = care.extraction_reference().unwrap();
        let value = secrets[reference].to_owned();
        let database = server(3);
        let mut attached =
            attach_postgres(&database, "erp", &care.postgres_source("erp", value)).unwrap();
        let clock = Recorded::new(clock_at(START));
        let mut brake = Brake::new(&care.budget());
        read_with_brake(
            &mut attached,
            "erp",
            &orders(),
            &chunks_of(2),
            &mut brake,
            &clock,
        )
        .unwrap();
        let secrets_created = database.params_of("CREATE TEMPORARY SECRET");
        assert_eq!(
            secrets_created,
            vec![vec![SqlValue::Text(expected.to_owned())]]
        );
        let reads = database.params_of("SELECT * FROM \"erp\".\"orders\"");
        assert_eq!(reads.len(), 2, "{:?}", database.statements());
    }
}

#[test]
fn mg_req_0427_connections_never_exceed_budget() {
    let table = [
        (Load::Low, 1, 10_000, Duration::from_secs(1)),
        (Load::Normal, 2, 50_000, Duration::from_millis(200)),
        (Load::High, 4, 100_000, Duration::ZERO),
    ];
    for (load, connections, chunk_rows, pause) in table {
        for max_connections in 1..=5_u32 {
            let options = json!({ "load": load.to_string(), "max_connections": max_connections });
            let care = SourceCare::of(&source(&options), "dev");
            let budget = care.budget();
            assert_eq!(
                budget,
                Budget {
                    connections: connections.min(max_connections),
                    chunk_rows: NonZeroU32::new(chunk_rows).unwrap(),
                    pause,
                }
            );
            let database = server(i64::from(chunk_rows) * 2 + 1);
            let pg = care.postgres_source("erp", "postgresql://primary/erp".to_owned());
            let mut attached = attach_postgres(&database, "erp", &pg).unwrap();
            let pool = database.params_of("SET pg_pool_max_connections");
            assert_eq!(
                pool,
                vec![vec![SqlValue::Integer(i64::from(budget.connections))]]
            );
            assert!(budget.connections <= max_connections);
            let clock = Recorded::new(clock_at(START));
            let mut brake = Brake::new(&budget);
            let options = chunks_of(budget.chunk_rows.get());
            read_with_brake(
                &mut attached,
                "erp",
                &orders(),
                &options,
                &mut brake,
                &clock,
            )
            .unwrap();
            let limits: Vec<_> = database
                .params_of("SELECT * FROM \"erp\".\"orders\"")
                .into_iter()
                .map(|params| params.last().cloned())
                .collect();
            let limit = Some(SqlValue::Integer(i64::from(chunk_rows)));
            assert_eq!(limits, vec![limit.clone(), limit.clone(), limit]);
            let expected: Vec<Duration> = if pause.is_zero() {
                vec![]
            } else {
                vec![pause; 2]
            };
            assert_eq!(clock.sleeps(), expected);
        }
    }
    let defaults = SourceCare::of(&source(&json!({})), "prod");
    assert_eq!((defaults.load, defaults.max_connections), (Load::Low, 2));
    assert_eq!(
        SourceCare::of(&source(&json!({})), "dev").load,
        Load::Normal
    );
    assert_eq!(defaults.query_timeout, Duration::from_secs(600));
}

/// A server whose `n`-th chunk statement takes `latencies[n]`, and fails when it is `None`.
fn timed(clock: Arc<SimClock>, latencies: Vec<Option<u64>>) -> Scripted {
    let sent = Arc::new(Mutex::new(0_usize));
    Scripted::new(move |_sql, params| {
        let Ok(mut count) = sent.lock() else {
            return Err(PortError::Failed("the script is poisoned".to_owned()));
        };
        let latency = latencies.get(*count).copied().flatten();
        *count += 1;
        let Some(millis) = latency else {
            return Err(PortError::Failed(
                "canceling statement due to statement timeout".to_owned(),
            ));
        };
        clock.advance(Duration::from_millis(millis));
        let after = match params.first() {
            Some(SqlValue::Integer(after)) if params.len() == 2 => *after,
            _ => 0,
        };
        Ok(ids(after + 1, 1))
    })
}

fn attached_to(database: &Scripted) -> Result<mergus_source::AttachedSource, PortError> {
    Ok(mergus_source::AttachedSource {
        connection: database.connect(&DatabaseLocation::InMemory)?,
        alias: "erp".to_owned(),
        identity: mergus_source::EngineIdentity {
            engine: mergus_source::DatabaseEngine::PostgreSql,
            version: "PostgreSQL 17.2".to_owned(),
        },
    })
}

#[test]
fn mg_req_0428_brake_slows_and_stops() {
    // Chunks of one row; a chunk that returns no row is the last, so the script ends with
    // a chunk past its latencies, which fails: the brake is read before that.
    let high = Load::High.table();
    let mut brake = Brake::new(&high);
    for latency in [10, 10, 10] {
        brake.chunk_read(Duration::from_millis(latency));
    }
    assert_eq!(brake.pause(), Duration::ZERO);
    brake.chunk_read(Duration::from_millis(30));
    assert_eq!(
        brake.pause(),
        Duration::from_millis(30),
        "slow chunk, pause from its latency"
    );
    brake.chunk_read(Duration::from_millis(40));
    assert_eq!(
        brake.pause(),
        Duration::from_millis(60),
        "slow again, pause doubled"
    );
    brake.chunk_read(Duration::from_millis(15));
    assert_eq!(
        brake.pause(),
        Duration::from_millis(60),
        "between median and twice: unchanged"
    );
    brake.chunk_read(Duration::from_millis(10));
    assert_eq!(
        brake.pause(),
        Duration::from_millis(30),
        "recovered: extra halved"
    );
    for _ in 0..10 {
        brake.chunk_read(Duration::from_millis(10));
    }
    assert_eq!(brake.pause(), Duration::ZERO, "back to the budget's pause");

    let low = Load::Low.table();
    let mut brake = Brake::new(&low);
    for latency in [10, 10, 10, 50] {
        brake.chunk_read(Duration::from_millis(latency));
    }
    assert_eq!(
        brake.pause(),
        Duration::from_secs(2),
        "the budget's pause doubled"
    );

    // Two failures then a success: the chunk is sent again and the read goes on.
    let clock = clock_at(START);
    let database = timed(Arc::clone(&clock), vec![Some(10), None, None, Some(10)]);
    let mut attached = attached_to(&database).unwrap();
    let recorded = Recorded::new(Arc::clone(&clock));
    let mut brake = Brake::new(&high);
    let incident = read_with_brake(
        &mut attached,
        "erp",
        &orders(),
        &chunks_of(1),
        &mut brake,
        &recorded,
    )
    .unwrap_err();
    // Chunks 5, 6, 7 have no script: three consecutive failures stop the source.
    assert_eq!(incident.code(), "MG-0405");
    assert_eq!(
        (incident.source_name.as_str(), incident.table.as_str()),
        ("erp", "orders")
    );
    assert_eq!(incident.failures, 3);
    assert_eq!(
        database.statements().len(),
        7,
        "4 scripted, then 3 failures and no more"
    );
    let message = incident.to_string();
    assert!(message.contains("source `erp`"), "{message}");
    assert!(message.contains("statement timeout"), "{message}");

    // A slow chunk is not a failure.
    let clock = clock_at(START);
    let database = timed(
        Arc::clone(&clock),
        vec![Some(10), Some(10), Some(500), Some(10), Some(10)],
    );
    let mut attached = attached_to(&database).unwrap();
    let recorded = Recorded::new(Arc::clone(&clock));
    let mut brake = Brake::new(&high);
    let _ = read_with_brake(
        &mut attached,
        "erp",
        &orders(),
        &chunks_of(1),
        &mut brake,
        &recorded,
    );
    let sleeps = recorded.sleeps();
    assert_eq!(
        sleeps.first(),
        Some(&Duration::from_millis(500)),
        "{sleeps:?}"
    );
    assert_eq!(
        sleeps.get(1),
        Some(&Duration::from_millis(250)),
        "{sleeps:?}"
    );
}

#[test]
fn mg_req_0430_full_loads_wait_for_window() {
    let night = Window::parse("01:00-03:00").unwrap();
    for (kind, waits) in [
        (LoadKind::Full, true),
        (LoadKind::Backfill, true),
        (LoadKind::Incremental, false),
    ] {
        let clock = clock_at(START);
        let waited = wait_for_window(&*clock, Some(&night), kind);
        let expected = if waits {
            // From 23:15:14 to 01:00:00.
            Duration::from_secs(3_600 + 44 * 60 + 46)
        } else {
            Duration::ZERO
        };
        assert_eq!(waited, expected, "{kind:?}");
        assert_eq!(night.contains(clock.now()), waits, "{kind:?}");
    }
    let clock = clock_at(START);
    assert_eq!(
        wait_for_window(&*clock, None, LoadKind::Full),
        Duration::ZERO
    );
    let across_midnight = Window::parse("22:00-02:00").unwrap();
    assert_eq!(
        wait_for_window(&*clock, Some(&across_midnight), LoadKind::Full),
        Duration::ZERO
    );
    let morning = Window::parse("23:16-23:20").unwrap();
    assert_eq!(
        wait_for_window(&*clock, Some(&morning), LoadKind::Full),
        Duration::from_secs(46)
    );
    assert_eq!(night.to_string(), "01:00-03:00");
    let care = SourceCare::of(&source(&json!({ "window": "01:00-03:00" })), "prod");
    assert_eq!(care.window, Some(night));
}
