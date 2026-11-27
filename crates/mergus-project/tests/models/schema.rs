//! MG-REQ-0604: the schema of a SQL step comes from DuckDB, with typed placeholders for the
//! run parameters, against empty input tables, so no data is read.

use std::collections::BTreeMap;

use mergus_ports::{ResultColumn, SqlValue};
use mergus_project::{LayoutError, ProjectError, read_sql_step, sql_schema};
use mergus_sim::{DatabaseFaults, SimDatabase};

use super::{PATH, known, parser, sql};

/// A column named `name` of DuckDB type `data_type`.
fn column(name: &str, data_type: &str) -> ResultColumn {
    ResultColumn {
        name: name.to_owned(),
        data_type: data_type.to_owned(),
    }
}

/// The columns of `raw.erp__orders`, the input of the fixture `parameters`.
fn orders() -> Vec<ResultColumn> {
    vec![
        column("order_id", "BIGINT"),
        column("updated_at", "TIMESTAMP WITH TIME ZONE"),
    ]
}

#[test]
fn mg_req_0604_sql_schema_by_prepare() {
    let text = sql("parameters");
    let step = read_sql_step(PATH, text.as_bytes(), &known(), &parser()).unwrap();
    // Recorded from the real database for the same text, inputs and placeholders.
    let columns = vec![
        column("order_id", "BIGINT"),
        column("updated_at", "TIMESTAMP WITH TIME ZONE"),
        column("run_date", "DATE"),
    ];
    let placeholders = vec![
        SqlValue::Date(0),
        SqlValue::TimestampTz(0),
        SqlValue::TimestampTz(0),
    ];
    let database = SimDatabase::new(0, DatabaseFaults::default());
    database.record_describe(text, placeholders.clone(), columns.clone());
    let inputs = BTreeMap::from([("raw.erp__orders".to_owned(), orders())]);

    assert_eq!(sql_schema(&database, &step, &inputs), Ok(columns));
    // Each input is an empty table with its schema; the step is described with a typed
    // placeholder for each parameter; nothing is run, appended or queried.
    assert_eq!(
        database.created(),
        [(Some("raw".to_owned()), "erp__orders".to_owned(), orders())]
    );
    assert_eq!(database.described(), [(text.to_owned(), placeholders)]);
    assert!(database.queried().is_empty());
    assert!(database.executed().is_empty());
    assert!(database.appended("erp__orders").is_empty());
}

#[test]
fn mg_req_0604_step_duckdb_refuses_is_mg_0601() {
    let text = sql("typed");
    let step = read_sql_step(PATH, text.as_bytes(), &known(), &parser()).unwrap();
    let database = SimDatabase::new(0, DatabaseFaults::default());
    let refusal = "Binder Error: Referenced column \"nope\" not found in FROM clause!";
    database.record_describe_failure(text, Vec::new(), refusal);
    let Err(LayoutError::Invalid(problems)) = sql_schema(&database, &step, &BTreeMap::new()) else {
        panic!("a step DuckDB refuses is a validation error");
    };
    assert_eq!(
        problems,
        [ProjectError::UnpreparableSql {
            path: PATH.to_owned(),
            message: refusal.to_owned(),
        }]
    );
    assert_eq!(problems[0].code(), "MG-0601");
    assert!(problems[0].to_string().contains(refusal), "{}", problems[0]);
}
