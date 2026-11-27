//! Tests of `describe` and `create_empty_table` of the real `Database` (MG-REQ-0604). The
//! schemas are those recorded in `crates/mergus-project/tests/models/schema.rs`.

use super::RealDatabase;
use crate::{Database, DatabaseConnection, DatabaseLocation, PortError, ResultColumn, SqlValue};

/// A column named `name` of DuckDB type `data_type`.
fn column(name: &str, data_type: &str) -> ResultColumn {
    ResultColumn {
        name: name.to_owned(),
        data_type: data_type.to_owned(),
    }
}

/// A connection to a new in-memory database with the empty table `raw.erp__orders`.
fn connection() -> Box<dyn DatabaseConnection> {
    let database = RealDatabase::new(&std::env::temp_dir().join("mergus-db-describe"));
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    let orders = [
        column("order_id", "BIGINT"),
        column("updated_at", "TIMESTAMP WITH TIME ZONE"),
    ];
    connection
        .create_empty_table(Some("raw"), "erp__orders", &orders)
        .unwrap();
    connection
}

#[test]
fn mg_req_0604_schema_without_data() {
    let mut connection = connection();
    let parameters = include_str!("../../../../mergus-project/tests/sql/parameters.sql");
    let placeholders = [
        SqlValue::Date(0),
        SqlValue::TimestampTz(0),
        SqlValue::TimestampTz(0),
    ];
    assert_eq!(
        connection.describe(parameters, &placeholders).unwrap(),
        [
            column("order_id", "BIGINT"),
            column("updated_at", "TIMESTAMP WITH TIME ZONE"),
            column("run_date", "DATE"),
        ]
    );
    let typed = include_str!("../../../../mergus-project/tests/sql/typed.sql");
    assert_eq!(
        connection.describe(typed, &[]).unwrap(),
        [
            column("order_id", "INTEGER"),
            column("amount", "DECIMAL(18,3)"),
            column("note", "VARCHAR"),
        ]
    );
    let refused = connection.describe("SELECT nope FROM raw.erp__orders", &[]);
    assert!(
        matches!(&refused, Err(PortError::Failed(message)) if message.contains("Binder Error")),
        "{refused:?}"
    );
}

#[test]
fn mg_req_0604_describe_runs_nothing() {
    let mut connection = connection();
    connection.execute("CREATE SEQUENCE seq", &[]).unwrap();
    assert_eq!(
        connection
            .describe("SELECT nextval('seq') AS n", &[])
            .unwrap(),
        [column("n", "BIGINT")]
    );
    let batches = connection
        .query_arrow("SELECT nextval('seq')::VARCHAR", &[])
        .unwrap();
    let next = arrow::array::cast::as_string_array(batches[0].column(0));
    assert_eq!(next.value(0), "1", "the sequence did not advance");
    let empty = connection
        .query_arrow("SELECT count(*)::VARCHAR FROM raw.erp__orders", &[])
        .unwrap();
    let count = arrow::array::cast::as_string_array(empty[0].column(0));
    assert_eq!(count.value(0), "0");
}
