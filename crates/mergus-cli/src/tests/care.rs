//! MG-REQ-0431 and MG-REQ-0432: `mergus doctor --source` warns for a key or incremental
//! column without an index, and `mergus plan` shows what reading each source table costs.
//! The PostgreSQL server is scripted: its answers are produced by an in-memory DuckDB.

use std::path::Path;
use std::time::Duration;

use mergus_ports::real::RealDatabase;
use mergus_ports::{Database, DatabaseLocation, Extension, SqlValue};
use mergus_sim::{DatabaseFaults, SimDatabase, SimRandom};
use mergus_source::{PG_ESTIMATES, PG_INDEXES, PG_TABLE_ESTIMATE, TABLE_INDEXES};

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
    database.record_execute(create, vec![text(REPLICA)], 0);
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

/// Records the indexes of `table`, each `(name, columns)`.
fn indexes(database: &SimDatabase, table: &str, indexes: &[(&str, &[&str])]) {
    let bound: Vec<SqlValue> = indexes
        .iter()
        .flat_map(|(index, columns)| {
            columns
                .iter()
                .flat_map(move |column| [text(index), text(column)])
        })
        .collect();
    let answer = if bound.is_empty() {
        "SELECT NULL::VARCHAR AS index_name, NULL::VARCHAR AS column_name WHERE false".to_owned()
    } else {
        let rows = vec!["(?::VARCHAR, ?::VARCHAR)"; bound.len() / 2].join(", ");
        "SELECT * FROM (VALUES ".to_owned() + &rows + ") AS t(index_name, column_name)"
    };
    let params = vec![text("erp"), text(PG_INDEXES), text("public"), text(table)];
    record(database, TABLE_INDEXES, params, (&answer, &bound));
}

/// Records the row estimate of `table`.
fn estimate(database: &SimDatabase, table: &str, rows: Option<i64>) {
    let bound: Vec<SqlValue> = rows.map(SqlValue::Integer).into_iter().collect();
    let answer = if bound.is_empty() {
        "SELECT 1::BIGINT AS estimate WHERE false"
    } else {
        "SELECT ?::BIGINT AS estimate"
    };
    let params = vec![text("erp"), text(PG_ESTIMATES), text("public"), text(table)];
    record(database, PG_TABLE_ESTIMATE, params, (answer, &bound));
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
    other_sources();
}

fn missing_indexes_are_warned() {
    let database = server(1);
    indexes(&database, "orders", &[("orders_pkey", &["id"])]);
    indexes(
        &database,
        "customers",
        &[("customers_idx", &["id", "region", "name"])],
    );
    indexes(&database, "items", &[("items_name", &["name", "item_id"])]);
    let outcome = run(ERP, &["doctor", "--source", "erp"], Some(&database));
    // Warnings never change the exit code (MG-REQ-0431.1).
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    let shown = format!("{}{}", outcome.stdout, outcome.stderr);
    assert_eq!(shown.matches("warning[MG-0406]").count(), 2, "{shown}");
    for suggestion in [
        "CREATE INDEX ON \"public\".\"orders\" (\"updated_at\");",
        "CREATE INDEX ON \"public\".\"items\" (\"item_id\");",
    ] {
        assert!(shown.contains(suggestion), "{shown}");
    }
    assert!(!shown.contains("\"customers\" ("), "{shown}");
    // The source is reached through its replica (MG-REQ-0426), and detached.
    let executed = database.executed();
    assert!(
        executed
            .iter()
            .any(|(_, params)| params == &vec![text(REPLICA)])
    );
    assert!(
        executed
            .iter()
            .all(|(_, params)| params != &vec![text(PRIMARY)])
    );
    assert!(executed.iter().any(|(sql, _)| sql == "DETACH \"erp\""));
}

fn present_indexes_are_not() {
    let database = server(1);
    indexes(
        &database,
        "orders",
        &[
            ("orders_pkey", &["id"]),
            ("orders_updated", &["updated_at"]),
        ],
    );
    indexes(
        &database,
        "customers",
        &[("customers_pkey", &["region", "id"])],
    );
    indexes(&database, "items", &[("items_pkey", &["item_id"])]);
    let outcome = run(ERP, &["doctor", "--source", "erp"], Some(&database));
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert!(!outcome.stderr.contains("MG-0406"), "{outcome:?}");
}

/// A DuckDB file source is not checked (MG-REQ-0431.3); an unknown source is an error.
fn other_sources() {
    let local = SimDatabase::new(1, DatabaseFaults::default());
    let attach_file = "ATTACH ? (READ_ONLY)";
    let file = format!("{ROOT}/data/local.duckdb");
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
    assert!(!outcome.stderr.contains("MG-0406"), "{outcome:?}");
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
    indexes(&database, "orders", &[("orders_updated", &["updated_at"])]);
    indexes(
        &database,
        "customers",
        &[("customers_pkey", &["region", "id"])],
    );
    indexes(&database, "items", &[]);
    estimate(&database, "orders", Some(12_000));
    estimate(&database, "customers", Some(340));
    estimate(&database, "items", None);
    let outcome = run(&erp, &["plan"], Some(&database));
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    let budget = "load high (3 connections, 100000 rows per chunk, pause 0 ms)";
    for line in [
        format!(
            "source erp.customers: estimated rows 340, index customers_pkey, window 01:00-03:00, {budget}"
        ),
        format!(
            "source erp.items: estimated rows unknown, index unknown, window 01:00-03:00, {budget}"
        ),
        format!(
            "source erp.orders: estimated rows 12000, index orders_updated, window 01:00-03:00, {budget}"
        ),
    ] {
        assert!(outcome.stdout.contains(&line), "{line}\n{}", outcome.stdout);
    }

    // Without a DuckDB, estimates and indexes are unknown; the default load of `prod` is
    // `low`, and `plan` does not fail.
    let outcome = run(ERP, &["plan"], None);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    let line = "source erp.orders: estimated rows unknown, index unknown, window none, \
                load low (1 connections, 10000 rows per chunk, pause 1000 ms)";
    assert!(outcome.stdout.contains(line), "{}", outcome.stdout);
}
