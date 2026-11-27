//! MG-REQ-0431 and MG-REQ-0432: `mergus doctor --source` warns for a key or incremental
//! column without an index, and `mergus plan` shows what reading each source table costs.
//! The PostgreSQL server is scripted: its answers are produced by an in-memory DuckDB.

use std::path::Path;
use std::time::Duration;

use mergus_ports::real::RealDatabase;
use mergus_ports::{Database, DatabaseLocation, Extension, SqlValue};
use mergus_sim::{DatabaseFaults, SimDatabase, SimRandom};
use mergus_source::{PG_ESTIMATES, PG_INDEXES, SCHEMA_ESTIMATES, SCHEMA_INDEXES};

use super::{Outcome, PROJECT, ROOT, Setup, project};

/// The value of `secret:erp`.
const PRIMARY: &str = "postgresql://primary/erp";

/// The value of `secret:erp_replica`.
const REPLICA: &str = "postgresql://replica/erp";

/// Records on `database` the answer to `sql` with `params`: the rows the real, in-memory
/// DuckDB returns for `rows` with `bound`. An error records no rows, which the test sees.
fn record(
    database: &SimDatabase,
    sql: &str,
    params: Vec<SqlValue>,
    (rows, bound): (&str, &[SqlValue]),
) {
    let real = RealDatabase::new(Path::new("/nonexistent"));
    let batches = real
        .connect(&DatabaseLocation::InMemory)
        .and_then(|mut connection| connection.query_arrow(rows, bound))
        .unwrap_or_default();
    database.record_query(sql, params, batches);
}

fn text(value: &str) -> SqlValue {
    SqlValue::Text(value.to_owned())
}

/// The URI `uri` as the temporary secret holds it, with the session parameters of
/// MG-REQ-0429.2.
fn protected(uri: &str) -> SqlValue {
    text(&format!(
        "{uri}?application_name=mergus&options=-c%20lock_timeout%3D5000"
    ))
}

/// A PostgreSQL server reached through the replica of `erp`, attached with a pool of
/// `connections` and the default statement timeout.
fn server(connections: i64) -> SimDatabase {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    database.record_extension(Extension::Postgres, "v1.4.0");
    let timeout = i64::try_from(Duration::from_secs(600).as_millis()).unwrap_or(i64::MAX);
    for (sql, value) in [
        (
            "SET pg_pool_max_connections = ?",
            SqlValue::Integer(connections),
        ),
        ("SET pg_pool_acquire_mode = ?", text("wait")),
        (
            "SET pg_statement_timeout_millis = ?",
            SqlValue::Integer(timeout),
        ),
    ] {
        database.record_execute(sql, vec![value], 0);
    }
    let create = "CREATE TEMPORARY SECRET \"mergus_source_erp\" (TYPE postgres, URI ?)";
    database.record_execute(create, vec![protected(REPLICA)], 0);
    let attach = "ATTACH '' AS \"erp\" (TYPE postgres, SECRET \"mergus_source_erp\", READ_ONLY)";
    database.record_execute(attach, Vec::new(), 0);
    database.record_execute("DETACH \"erp\"", Vec::new(), 0);
    let drop = "DROP TEMPORARY SECRET IF EXISTS \"mergus_source_erp\"";
    database.record_execute(drop, Vec::new(), 0);
    record(
        &database,
        "SELECT * FROM postgres_query(?, ?)",
        vec![text("erp"), text("SELECT version()")],
        ("SELECT 'PostgreSQL 17.2' AS version", &[]),
    );
    database
}

/// The indexes of each table: `(table, [(index, [columns])])`.
type Indexes<'a> = &'a [(&'a str, &'a [(&'a str, &'a [&'a str])])];

/// Records the indexes of the schema `public`, read once per source (MG-REQ-0431.3).
fn indexes(database: &SimDatabase, tables: Indexes<'_>) {
    let bound: Vec<SqlValue> = tables
        .iter()
        .flat_map(|(table, indexes)| {
            indexes.iter().flat_map(move |(index, columns)| {
                columns
                    .iter()
                    .flat_map(move |column| [text(table), text(index), text(column)])
            })
        })
        .collect();
    let answer = if bound.is_empty() {
        "SELECT NULL::VARCHAR AS table_name, NULL::VARCHAR AS index_name, \
         NULL::VARCHAR AS column_name WHERE false"
            .to_owned()
    } else {
        let rows = vec!["(?::VARCHAR, ?::VARCHAR, ?::VARCHAR)"; bound.len() / 3].join(", ");
        "SELECT * FROM (VALUES ".to_owned() + &rows + ") AS t(table_name, index_name, column_name)"
    };
    let params = vec![text("erp"), text(PG_INDEXES), text("public")];
    record(database, SCHEMA_INDEXES, params, (&answer, &bound));
}

/// Records the row estimates of the schema `public`, read once per source.
fn estimates(database: &SimDatabase, tables: &[(&str, i64)]) {
    let bound: Vec<SqlValue> = tables
        .iter()
        .flat_map(|(table, rows)| [text(table), SqlValue::Integer(*rows)])
        .collect();
    let rows = vec!["(?::VARCHAR, ?::BIGINT)"; tables.len()].join(", ");
    let answer = "SELECT * FROM (VALUES ".to_owned() + &rows + ") AS t(table_name, estimate)";
    let params = vec![text("erp"), text(PG_ESTIMATES), text("public")];
    record(database, SCHEMA_ESTIMATES, params, (&answer, &bound));
}

/// Runs `mergus <args>` on a project whose source `erp` is `erp` and whose source `local`
/// is a DuckDB file, with both secrets of `erp` in the environment.
fn run(erp: &str, args: &[&str], database: Option<&dyn Database>) -> Outcome {
    let files = project(&[
        ("project.toml", PROJECT),
        ("sources/erp.toml", erp),
        (
            "sources/local.toml",
            "kind = \"database\"\nengine = \"duckdb\"\nlocation = \"data/local.duckdb\"\n",
        ),
    ]);
    let erp_var = mergus_security::SecretRef::from_name("erp")
        .unwrap()
        .variable();
    let replica_var = mergus_security::SecretRef::from_name("erp_replica")
        .unwrap()
        .variable();
    let vars = [(erp_var.as_str(), PRIMARY), (replica_var.as_str(), REPLICA)];
    let random = SimRandom::new(7);
    let setup = Setup {
        files: &files,
        cwd: ROOT,
        args,
        vars: &vars,
        random: &random,
        seed: 7,
        terminal: false,
        input: None,
    };
    match database {
        Some(database) => setup.run_with_database(database),
        None => setup.run(),
    }
}

const ERP: &str = "kind = \"database\"\nengine = \"postgresql\"\nconnection = \"secret:erp\"\n\
     replica = \"secret:erp_replica\"\n\
     [tables.orders]\nkey = \"id\"\nincremental = \"updated_at\"\ndeletes = \"none\"\n\
     [tables.customers]\nkey = [\"region\", \"id\"]\n\
     [tables.items]\nkey = \"item_id\"\n";

#[test]
fn mg_req_0431_doctor_reports_missing_index() {
    missing_indexes_are_warned();
    present_indexes_are_not();
    unreachable_source_is_typed();
    other_sources();
}

fn missing_indexes_are_warned() {
    let database = server(1);
    // `customers_idx` holds the key columns in another order, and `items_name` does not
    // lead with the key: neither counts (MG-REQ-0431.4).
    indexes(
        &database,
        &[
            ("orders", &[("orders_pkey", &["id"])]),
            ("customers", &[("customers_idx", &["id", "region", "name"])]),
            ("items", &[("items_name", &["name", "item_id"])]),
        ],
    );
    let outcome = run(ERP, &["doctor", "--source", "erp"], Some(&database));
    // Warnings never change the exit code (MG-REQ-0431.1).
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    let shown = format!("{}{}", outcome.stdout, outcome.stderr);
    assert_eq!(shown.matches("warning[MG-0407]").count(), 3, "{shown}");
    for suggestion in [
        "CREATE INDEX ON \"public\".\"orders\" (\"updated_at\");",
        "CREATE INDEX ON \"public\".\"customers\" (\"region\", \"id\");",
        "CREATE INDEX ON \"public\".\"items\" (\"item_id\");",
    ] {
        assert!(shown.contains(suggestion), "{shown}");
    }
    // The source is reached through its replica (MG-REQ-0426), and detached.
    let executed = database.executed();
    assert!(
        executed
            .iter()
            .any(|(_, params)| params == &vec![protected(REPLICA)])
    );
    assert!(
        executed
            .iter()
            .all(|(_, params)| params != &vec![protected(PRIMARY)])
    );
    assert!(executed.iter().any(|(sql, _)| sql == "DETACH \"erp\""));
}

fn present_indexes_are_not() {
    let database = server(1);
    indexes(
        &database,
        &[
            (
                "orders",
                &[
                    ("orders_pkey", &["id"]),
                    ("orders_updated", &["updated_at"]),
                ],
            ),
            (
                "customers",
                &[("customers_pkey", &["region", "id", "name"])],
            ),
            ("items", &[("items_pkey", &["item_id"])]),
        ],
    );
    let outcome = run(ERP, &["doctor", "--source", "erp"], Some(&database));
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert!(!outcome.stderr.contains("MG-0407"), "{outcome:?}");
}

/// A source that cannot be reached is the error `MG-0408` (MG-REQ-0008).
fn unreachable_source_is_typed() {
    let outcome = run(ERP, &["doctor", "--source", "erp"], None);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert!(outcome.stderr.contains("error[MG-0408]"), "{outcome:?}");
    assert!(
        outcome.stderr.contains("source `erp` cannot be reached"),
        "{outcome:?}"
    );
    // The catalog cannot be read: nothing recorded answers `pg_index`.
    let outcome = run(ERP, &["doctor", "--source", "erp"], Some(&server(1)));
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert!(outcome.stderr.contains("error[MG-0408]"), "{outcome:?}");
    assert!(
        outcome.stderr.contains("catalog of source `erp`"),
        "{outcome:?}"
    );
}

/// A DuckDB file source is not checked (MG-REQ-0431.3); an unknown source is an error.
fn other_sources() {
    let local = SimDatabase::new(1, DatabaseFaults::default());
    let attach_file = "ATTACH ? (READ_ONLY)";
    // The path is built as the binary builds it, with the separator of the platform.
    let file = Path::new(ROOT)
        .join("data/local.duckdb")
        .display()
        .to_string();
    local.record_execute(attach_file, vec![text(&file)], 0);
    record(
        &local,
        "SELECT version()",
        Vec::new(),
        ("SELECT 'v1.4.2' AS version", &[]),
    );
    local.record_execute("DETACH \"local\"", Vec::new(), 0);
    local.record_execute(
        "DROP TEMPORARY SECRET IF EXISTS \"mergus_source_local\"",
        Vec::new(),
        0,
    );
    let outcome = run(ERP, &["doctor", "--source", "local"], Some(&local));
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert!(!outcome.stderr.contains("MG-0407"), "{outcome:?}");
    assert!(local.executed().iter().any(|(sql, _)| sql == attach_file));
    let outcome = run(ERP, &["doctor", "--source", "erq"], Some(&server(1)));
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert!(outcome.stderr.contains("MG-1310"), "{outcome:?}");
}

#[test]
fn mg_req_0432_plan_shows_source_impact() {
    let erp = ERP.replacen(
        "replica = \"secret:erp_replica\"\n",
        "replica = \"secret:erp_replica\"\nload = \"high\"\nmax_connections = 3\nwindow = \"01:00-03:00\"\n",
        1,
    );
    let database = server(3);
    indexes(
        &database,
        &[
            ("orders", &[("orders_updated", &["updated_at"])]),
            ("customers", &[("customers_pkey", &["region", "id"])]),
        ],
    );
    estimates(&database, &[("orders", 12_000), ("customers", 340)]);
    let outcome = run(&erp, &["plan"], Some(&database));
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    let budget = "load high (3 connections, 100000 rows per chunk, pause 0 ms)";
    for line in [
        format!(
            "source erp.customers: estimated rows up to 340, index customers_pkey, window 01:00-03:00, {budget}"
        ),
        format!(
            "source erp.items: estimated rows unknown, index unknown, window 01:00-03:00, {budget}"
        ),
        format!(
            "source erp.orders: estimated rows up to 12000, index orders_updated, window 01:00-03:00, {budget}"
        ),
    ] {
        assert!(outcome.stdout.contains(&line), "{line}\n{}", outcome.stdout);
    }

    assert!(!outcome.stderr.contains("MG-0409"), "{outcome:?}");
    plan_warns_for_an_unreachable_source();
}

/// Without a DuckDB, estimates and indexes are unknown and the reason is the warning
/// `MG-0409`; the default load of `prod` is `low`, and `plan` does not fail
/// (MG-REQ-0432.2).
fn plan_warns_for_an_unreachable_source() {
    let outcome = run(ERP, &["plan"], None);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    let line = "source erp.orders: estimated rows unknown, index unknown, window none, \
                load low (1 connections, 10000 rows per chunk, pause 1000 ms)";
    assert!(outcome.stdout.contains(line), "{}", outcome.stdout);
    assert!(outcome.stderr.contains("warning[MG-0409]"), "{outcome:?}");
    assert!(
        outcome
            .stderr
            .contains("MG-0408 source `erp` cannot be reached"),
        "{outcome:?}"
    );
}
