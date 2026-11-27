//! MG-REQ-0421 and MG-REQ-0410: database sources are attached read-only through the
//! simulated `Database` port, and the engine that answers is identified before any data is
//! read.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arrow::array::{ArrayRef, StringArray};
use arrow::record_batch::RecordBatch;
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, Extension, PortError, PostgresSource, SqlValue,
};
use mergus_sim::{DatabaseFaults, SimDatabase};
use mergus_source::{ConnectError, DatabaseEngine, attach_duckdb_file, attach_postgres};

const FILE: &str = "/project/data/erp.duckdb";
const ATTACH: &str = "ATTACH ? (READ_ONLY)";
const DUCKDB_VERSION: &str = "v1.4.2";
const PG_VERSION: &str = "SELECT * FROM postgres_query(?, ?)";
const POSTGRESQL: &str = "PostgreSQL 17.2 on x86_64-pc-linux-gnu";

/// A database that records the location of every connection it opens.
struct Recording {
    inner: SimDatabase,
    locations: Mutex<Vec<DatabaseLocation>>,
}

impl Database for Recording {
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

fn text(value: &str) -> Vec<RecordBatch> {
    let column: ArrayRef = Arc::new(StringArray::from(vec![value]));
    // An error leaves no batch, which every test that reads the answer would see.
    RecordBatch::try_from_iter([("version", column)])
        .into_iter()
        .collect()
}

fn sim() -> SimDatabase {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    database.record_query("SELECT version()", Vec::new(), text(DUCKDB_VERSION));
    database
}

fn duckdb_file(database: &SimDatabase) {
    database.record_execute(ATTACH, vec![SqlValue::Text(FILE.to_owned())], 0);
}

fn postgres() -> PostgresSource {
    PostgresSource {
        alias: "erp".to_owned(),
        connection: "postgresql://reader@db/erp".to_owned(),
        max_connections: 2,
        query_timeout: Duration::from_secs(30),
    }
}

/// Records what the PostgreSQL attach of [`postgres`] runs, and `answer` to `version()`.
fn postgres_server(database: &SimDatabase, answer: Option<&str>) {
    database.record_extension(Extension::Postgres, "v1.4.0");
    for (sql, value) in [
        ("SET pg_pool_max_connections = ?", SqlValue::Integer(2)),
        (
            "SET pg_pool_acquire_mode = ?",
            SqlValue::Text("wait".to_owned()),
        ),
        (
            "SET pg_statement_timeout_millis = ?",
            SqlValue::Integer(30_000),
        ),
    ] {
        database.record_execute(sql, vec![value], 0);
    }
    // The secret holds the value with the session parameters of MG-REQ-0429.2.
    let uri = SqlValue::Text(format!(
        "{}?application_name=mergus&options=-c%20lock_timeout%3D5000",
        postgres().connection
    ));
    let create = "CREATE TEMPORARY SECRET \"mergus_source_erp\" (TYPE postgres, URI ?)";
    database.record_execute(create, vec![uri], 0);
    let attach = "ATTACH '' AS \"erp\" (TYPE postgres, SECRET \"mergus_source_erp\", READ_ONLY)";
    database.record_execute(attach, Vec::new(), 0);
    database.record_execute("DETACH \"erp\"", Vec::new(), 0);
    let drop = "DROP TEMPORARY SECRET IF EXISTS \"mergus_source_erp\"";
    database.record_execute(drop, Vec::new(), 0);
    if let Some(answer) = answer {
        let params = vec![
            SqlValue::Text("erp".to_owned()),
            SqlValue::Text("SELECT version()".to_owned()),
        ];
        database.record_query(PG_VERSION, params, text(answer));
    }
}

#[test]
fn mg_req_0421_duckdb_file_read_only() {
    let inner = sim();
    duckdb_file(&inner);
    let database = Recording {
        inner: inner.clone(),
        locations: Mutex::new(Vec::new()),
    };
    let source = attach_duckdb_file(&database, "erp", Path::new(FILE)).unwrap();
    assert_eq!(source.alias, "erp");
    assert_eq!(source.identity.engine, DatabaseEngine::DuckDb);
    assert_eq!(source.identity.version, DUCKDB_VERSION);

    // The connection keeps its own database in memory: the file is only attached, read-only,
    // and nothing else is run, so no write reaches it.
    assert_eq!(
        *database.locations.lock().unwrap(),
        [DatabaseLocation::InMemory]
    );
    let attach = (ATTACH.to_owned(), vec![SqlValue::Text(FILE.to_owned())]);
    assert_eq!(inner.executed(), [attach]);
    assert!(inner.appended("erp").is_empty());
    assert!(inner.attached().is_empty());
}

/// The source name, declared engine and detected engine of a mismatch.
fn mismatch(error: &ConnectError) -> Option<(&str, DatabaseEngine, Option<&str>)> {
    match error {
        ConnectError::EngineMismatch {
            source_name,
            declared,
            detected,
            ..
        } => Some((source_name.as_str(), *declared, detected.as_deref())),
        ConnectError::Port(_) => None,
    }
}

#[test]
fn mg_req_0410_connected_engine_matches_declared() {
    let database = sim();
    duckdb_file(&database);
    let file = attach_duckdb_file(&database, "erp", Path::new(FILE)).unwrap();
    assert_eq!(file.identity.engine, DatabaseEngine::DuckDb);
    assert_eq!(file.identity.version, DUCKDB_VERSION);

    let database = sim();
    postgres_server(&database, Some(POSTGRESQL));
    let source = attach_postgres(&database, "erp", &postgres()).unwrap();
    assert_eq!(source.alias, "erp");
    assert_eq!(source.identity.engine, DatabaseEngine::PostgreSql);
    assert_eq!(source.identity.version, POSTGRESQL);
    let executed = database.executed();
    assert!(!executed.iter().any(|(sql, _)| sql.starts_with("DETACH")));
}

/// A DuckDB file is identified by its attach without `TYPE`; a file DuckDB refuses cannot be
/// identified, and nothing else runs.
#[test]
fn mg_req_0410_duckdb_file_refused_is_unidentified() {
    let database = sim();
    let error = attach_duckdb_file(&database, "erp", Path::new(FILE))
        .err()
        .unwrap();
    assert_eq!(error.code(), Some("MG-0404"));
    assert_eq!(
        mismatch(&error),
        Some(("erp", DatabaseEngine::DuckDb, None))
    );
    let message = error.to_string();
    assert!(
        message.contains("source `erp` declares engine `duckdb`")
            && message.contains("cannot be identified"),
        "{message}"
    );
    assert_eq!(database.executed().len(), 1);
}

/// An answer that does not start with `PostgreSQL` is another engine; a query that fails
/// leaves the engine unidentified. Either way the source is detached and its secret dropped.
#[test]
fn mg_req_0410_postgresql_mismatch_and_unidentified() {
    let cases = [
        (
            Some("CockroachDB CCL v24.1.0"),
            Some("CockroachDB CCL v24.1.0"),
        ),
        (Some("postgresql 17"), Some("postgresql 17")),
        (None, None),
    ];
    for (answer, expected) in cases {
        let database = sim();
        postgres_server(&database, answer);
        let error = attach_postgres(&database, "erp", &postgres())
            .err()
            .unwrap();
        assert_eq!(error.code(), Some("MG-0404"), "{error}");
        assert_eq!(
            mismatch(&error),
            Some(("erp", DatabaseEngine::PostgreSql, expected))
        );
        let message = error.to_string();
        assert!(
            message.contains("source `erp` declares engine `postgresql`"),
            "{message}"
        );
        if let Some(expected) = expected {
            assert!(message.contains(&format!("`{expected}`")), "{message}");
        }
        let executed: Vec<String> = database
            .executed()
            .into_iter()
            .map(|(sql, _)| sql)
            .collect();
        assert_eq!(
            executed[executed.len() - 2..],
            [
                "DETACH \"erp\"".to_owned(),
                "DROP TEMPORARY SECRET IF EXISTS \"mergus_source_erp\"".to_owned()
            ]
        );
    }
}

#[test]
fn mg_req_0410_port_failures_are_not_mismatches() {
    let database = sim();
    let error = attach_postgres(&database, "erp", &postgres())
        .err()
        .unwrap();
    assert!(matches!(error, ConnectError::Port(_)), "{error:?}");
    assert_eq!(error.code(), None);
}
