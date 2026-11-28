//! MG-REQ-0194 and MG-REQ-0196: the simulated `Database` answers from results recorded with
//! the real one and injects errors, deterministically for a seed.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{ArrayRef, Int64Array};
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use mergus_ports::{
    Database, DatabaseLocation, DuckLakeAccess, DuckLakeCatalog, Extension, LoadedExtension,
    PortError, PostgresSource, ResultColumn, SqlValue,
};
use mergus_sim::{DatabaseFaults, Rate, SimDatabase};

fn batch() -> Result<RecordBatch, ArrowError> {
    let ids: ArrayRef = Arc::new(Int64Array::from(vec![7]));
    RecordBatch::try_from_iter([("id", ids)])
}

#[test]
fn mg_req_0194_simulated_database_replays_recorded_results() {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let params = vec![SqlValue::Integer(7)];
    database.record_execute("INSERT INTO items VALUES (?)", params.clone(), 1);
    database.record_query("SELECT id FROM items", Vec::new(), vec![batch().unwrap()]);
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();

    let insert = "INSERT INTO items VALUES (?)";
    assert_eq!(connection.execute(insert, &params).unwrap(), 1);
    let other = [SqlValue::Integer(8)];
    assert!(matches!(
        connection.execute(insert, &other),
        Err(PortError::NotFound(_))
    ));
    let rows = connection.query_arrow("SELECT id FROM items", &[]).unwrap();
    assert_eq!(rows, vec![batch().unwrap()]);
    assert!(matches!(
        connection.execute("SELECT id FROM items", &[]),
        Err(PortError::NotFound(_))
    ));

    connection.append_arrow("items", &batch().unwrap()).unwrap();
    assert_eq!(database.appended("items"), vec![batch().unwrap()]);
    let catalog = DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: "metadata.ducklake".to_owned(),
        data_path: "data/".to_owned(),
    };
    connection.attach_ducklake(&catalog).unwrap();
    assert_eq!(database.attached(), vec![catalog]);
}

#[test]
fn mg_req_0515_simulated_database_attaches_encrypted() {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    let catalog = DuckLakeCatalog {
        alias: "raw".to_owned(),
        metadata: "raw.ducklake".to_owned(),
        data_path: "data/".to_owned(),
    };
    let access = DuckLakeAccess {
        read_only: true,
        snapshot: Some(3),
    };
    connection
        .attach_ducklake_encrypted(&catalog, &access)
        .unwrap();
    assert_eq!(database.attached_encrypted(), vec![catalog.clone()]);
    assert_eq!(database.encrypted_access(), vec![access]);
    assert!(database.attached().is_empty());

    // A catalog recorded as unencrypted is refused, as DuckLake refuses it.
    database.record_unencrypted("raw.ducklake");
    assert_eq!(
        connection.attach_ducklake_encrypted(&catalog, &access),
        Err(PortError::UnencryptedCatalog {
            catalog: "raw.ducklake".to_owned()
        })
    );
    assert_eq!(database.attached_encrypted().len(), 1);
}

#[test]
fn mg_req_0194_simulated_database_injects_errors_for_a_seed() {
    let outcomes = |seed| {
        let faults = DatabaseFaults {
            error: Rate::per_million(500_000),
        };
        let database = SimDatabase::new(seed, faults);
        database.record_execute("DELETE FROM items", Vec::new(), 0);
        let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
        (0..64)
            .map(|_| connection.execute("DELETE FROM items", &[]).is_ok())
            .collect::<Vec<_>>()
    };
    let run = outcomes(3);
    assert_eq!(run, outcomes(3));
    assert!(run.contains(&true) && run.contains(&false));

    let always = SimDatabase::new(
        3,
        DatabaseFaults {
            error: Rate::ALWAYS,
        },
    );
    let mut connection = always.connect(&DatabaseLocation::InMemory).unwrap();
    assert!(matches!(
        connection.append_arrow("items", &batch().unwrap()),
        Err(PortError::Failed(_))
    ));
}

/// The statements a PostgreSQL source is attached and detached with, in order, as the real
/// `Database` runs them (MG-REQ-0196.2 and MG-REQ-0196.3).
fn postgres_statements(connection: &str) -> Vec<(&'static str, Vec<SqlValue>)> {
    vec![
        (
            "SET pg_pool_max_connections = ?",
            vec![SqlValue::Integer(2)],
        ),
        (
            "SET pg_pool_acquire_mode = ?",
            vec![SqlValue::Text("wait".to_owned())],
        ),
        (
            "SET pg_statement_timeout_millis = ?",
            vec![SqlValue::Integer(600_000)],
        ),
        (
            "CREATE TEMPORARY SECRET \"mergus_source_crm\" (TYPE postgres, URI ?)",
            vec![SqlValue::Text(connection.to_owned())],
        ),
        (
            "ATTACH '' AS \"crm\" (TYPE postgres, SECRET \"mergus_source_crm\", READ_ONLY)",
            Vec::new(),
        ),
    ]
}

/// The statements the source `crm` is detached with.
fn detach_statements() -> [(&'static str, Vec<SqlValue>); 2] {
    [
        ("DETACH \"crm\"", Vec::new()),
        (
            "DROP TEMPORARY SECRET IF EXISTS \"mergus_source_crm\"",
            Vec::new(),
        ),
    ]
}

/// The connection value of `crm` with the session parameters of MG-REQ-0429.2, as the secret
/// holds it.
const CRM_PROTECTED: &str = "postgresql://reader:pw@db.example/crm?application_name=mergus&options=-c%20lock_timeout%3D5000";

fn crm(connection: &str) -> PostgresSource {
    PostgresSource {
        alias: "crm".to_owned(),
        connection: connection.to_owned(),
        max_connections: 2,
        query_timeout: Duration::from_secs(600),
    }
}

#[test]
fn mg_req_0196_simulated_database_attaches_sources_read_only() {
    let connection_value = "postgresql://reader:pw@db.example/crm";
    let database = SimDatabase::new(1, DatabaseFaults::default());
    database.record_extension(Extension::Postgres, "8b9d0bbe8d");
    let statements = postgres_statements(CRM_PROTECTED);
    for (sql, params) in &statements {
        database.record_execute(*sql, params.clone(), 0);
    }
    let detach = detach_statements();
    for (sql, params) in &detach {
        database.record_execute(*sql, params.clone(), 0);
    }
    let file = Path::new("data/sales.duckdb");
    let file_params = vec![SqlValue::Text("data/sales.duckdb".to_owned())];
    database.record_execute("ATTACH ? (READ_ONLY)", file_params.clone(), 0);
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();

    // PostgreSQL: the pool and timeout settings, then a temporary secret, then a read-only
    // attach that names the secret and never holds the connection value.
    connection.attach_postgres(&crm(connection_value)).unwrap();
    let owned = |list: &[(&str, Vec<SqlValue>)]| {
        list.iter()
            .map(|(sql, params)| ((*sql).to_owned(), params.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(database.executed(), owned(&statements));
    assert!(!statements[4].0.contains(connection_value));
    assert_eq!(
        connection.loaded_extensions(),
        vec![LoadedExtension {
            extension: Extension::Postgres,
            version: "8b9d0bbe8d".to_owned(),
        }]
    );
    // The source's debug form never shows the connection value.
    assert!(!format!("{:?}", crm(connection_value)).contains(connection_value));

    // Detaching drops the secret.
    connection.detach("crm").unwrap();
    let mut expected = owned(&statements);
    expected.extend(owned(&detach));
    assert_eq!(database.executed(), expected);

    // A DuckDB file: attached read-only, with no extension.
    connection.attach_duckdb_file(file).unwrap();
    expected.push(("ATTACH ? (READ_ONLY)".to_owned(), file_params));
    assert_eq!(database.executed(), expected);
    assert_eq!(connection.loaded_extensions().len(), 1);
}

#[test]
fn mg_req_0196_simulated_failed_attach_drops_the_secret() {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    database.record_extension(Extension::Postgres, "8b9d0bbe8d");
    let statements = postgres_statements(
        "host=db.example application_name=mergus options='-c lock_timeout=5000'",
    );
    // The attach itself has no recorded result, so it fails.
    for (sql, params) in &statements[..4] {
        database.record_execute(*sql, params.clone(), 0);
    }
    let drop = "DROP TEMPORARY SECRET IF EXISTS \"mergus_source_crm\"";
    database.record_execute(drop, Vec::new(), 0);
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    assert!(connection.attach_postgres(&crm("host=db.example")).is_err());
    let executed = database.executed();
    assert_eq!(executed.len(), 6);
    assert_eq!(executed[5], (drop.to_owned(), Vec::new()));

    // A missing extension fails before any statement.
    let bare = SimDatabase::new(1, DatabaseFaults::default());
    let mut connection = bare.connect(&DatabaseLocation::InMemory).unwrap();
    assert!(connection.attach_postgres(&crm("host=db.example")).is_err());
    assert!(bare.executed().is_empty());
}

/// A statement as the simulated database executed it, with its bound values.
type Statement = (String, Vec<SqlValue>);

/// Records the statements of attaching `crm` with the connection value `sent`, and attaches
/// `given`; returns the value bound into the temporary secret.
fn bound_secret_value(given: &str, sent: &str) -> Result<(Vec<Statement>, SqlValue), PortError> {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    database.record_extension(Extension::Postgres, "8b9d0bbe8d");
    for (sql, params) in postgres_statements(sent) {
        database.record_execute(sql, params, 0);
    }
    let mut connection = database.connect(&DatabaseLocation::InMemory)?;
    connection.attach_postgres(&crm(given))?;
    let executed = database.executed();
    let secret = executed[3].1[0].clone();
    Ok((executed, secret))
}

#[test]
fn mg_req_0429_session_is_read_only_and_identified() {
    // URI form: percent-encoded query parameters, after `?` or `&`.
    for (given, sent) in [
        (
            "postgresql://reader:pw@db.example/crm",
            "postgresql://reader:pw@db.example/crm?application_name=mergus\
             &options=-c%20lock_timeout%3D5000",
        ),
        (
            "postgres://db.example/crm?sslmode=require",
            "postgres://db.example/crm?sslmode=require&application_name=mergus\
             &options=-c%20lock_timeout%3D5000",
        ),
        (
            "postgresql://db.example/crm?",
            "postgresql://db.example/crm?application_name=mergus\
             &options=-c%20lock_timeout%3D5000",
        ),
        // Keyword/value form: two more keywords, the value of `options` quoted.
        (
            "host=db.example dbname=crm password='a b'",
            "host=db.example dbname=crm password='a b' application_name=mergus \
             options='-c lock_timeout=5000'",
        ),
    ] {
        let (executed, secret) = bound_secret_value(given, sent).unwrap();
        assert_eq!(secret, SqlValue::Text(sent.to_owned()));
        // Read-only attach, statement timeout from `query_timeout`, never the value in SQL.
        assert!(executed[4].0.ends_with("READ_ONLY)"), "{}", executed[4].0);
        assert_eq!(
            executed[2],
            (
                "SET pg_statement_timeout_millis = ?".to_owned(),
                vec![SqlValue::Integer(600_000)]
            )
        );
        assert!(executed.iter().all(|(sql, _)| !sql.contains("db.example")));
    }
}

#[test]
fn mg_req_0429_session_parameter_already_set_is_refused() {
    // A value that already sets either parameter is refused before any statement, with the
    // code MG-0410, the source and the parameter, and never the value.
    for (value, parameter) in [
        (
            "postgresql://db.example/crm?application_name=etl",
            "application_name",
        ),
        (
            "postgresql://db.example/crm?sslmode=require&options=-c%20x%3D1",
            "options",
        ),
        (
            "postgresql://db.example/crm?application%5Fname=etl",
            "application_name",
        ),
        ("host=db.example application_name=etl", "application_name"),
        ("host=db.example options = '-c x=1'", "options"),
    ] {
        let database = SimDatabase::new(1, DatabaseFaults::default());
        database.record_extension(Extension::Postgres, "8b9d0bbe8d");
        let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
        let error = connection.attach_postgres(&crm(value)).unwrap_err();
        assert_eq!(
            error,
            PortError::SessionParameterSet {
                source_name: "crm".to_owned(),
                parameter: parameter.to_owned(),
            },
            "{value}"
        );
        assert_eq!(error.code(), Some("MG-0410"));
        let message = error.to_string();
        assert!(
            message.contains("`crm`") && message.contains(parameter),
            "{message}"
        );
        assert!(
            !message.contains("db.example") && !message.contains("etl"),
            "{message}"
        );
        assert!(database.executed().is_empty());
    }
}

#[test]
fn simulated_database_replays_described_schemas() {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let columns = vec![ResultColumn {
        name: "id".to_owned(),
        data_type: "BIGINT".to_owned(),
    }];
    let params = vec![SqlValue::Date(0)];
    database.record_describe("SELECT id FROM items", params.clone(), columns.clone());
    database.record_describe_failure("SELECT nope", Vec::new(), "Binder Error");
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    connection
        .create_empty_table(Some("raw"), "items", &columns)
        .unwrap();
    let described = connection.describe("SELECT id FROM items", &params);
    assert_eq!(described.unwrap(), columns);
    assert_eq!(
        connection.describe("SELECT nope", &[]),
        Err(PortError::Refused("Binder Error".to_owned()))
    );
    assert!(matches!(
        connection.describe("SELECT id FROM items", &[]),
        Err(PortError::NotFound(_))
    ));
    assert_eq!(database.described().len(), 3);
    assert_eq!(
        database.created(),
        [(Some("raw".to_owned()), "items".to_owned(), columns)]
    );
    assert!(connection.query_arrow("SELECT 3", &[]).is_err());
    assert_eq!(database.queried(), [("SELECT 3".to_owned(), Vec::new())]);
    assert!(database.executed().is_empty());
}
