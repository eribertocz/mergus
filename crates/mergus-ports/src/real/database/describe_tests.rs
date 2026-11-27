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
    // The statement `mergus-project` makes of its fixture `parameters`.
    let parameters = "SELECT *, $run_date AS run_date\nFROM raw.erp__orders\n\
         WHERE updated_at >= CAST($period_start AS TIMESTAMPTZ) AND updated_at < \
         CAST($period_end AS TIMESTAMPTZ)\n";
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
}

#[test]
fn mg_req_0604_period_parameters_are_timestamptz() {
    let mut connection = connection();
    let placeholders = [SqlValue::TimestampTz(0), SqlValue::Date(0)];
    assert_eq!(
        connection
            .describe(
                "SELECT CAST($period_start AS TIMESTAMPTZ) AS p, $run_date AS d",
                &placeholders
            )
            .unwrap(),
        [column("p", "TIMESTAMP WITH TIME ZONE"), column("d", "DATE")]
    );
    // The statement `mergus-project` makes of its fixture `period_output`.
    let output = "SELECT CAST($period_start AS TIMESTAMPTZ) AS period_start, \
         CAST($period_end AS TIMESTAMPTZ) AS period_end, $run_date AS run_date\n";
    let placeholders = [
        SqlValue::TimestampTz(0),
        SqlValue::TimestampTz(0),
        SqlValue::Date(0),
    ];
    assert_eq!(
        connection.describe(output, &placeholders).unwrap(),
        [
            column("period_start", "TIMESTAMP WITH TIME ZONE"),
            column("period_end", "TIMESTAMP WITH TIME ZONE"),
            column("run_date", "DATE"),
        ]
    );
    // The bound instant is kept: without `icu`, DuckDB casts it as UTC.
    let instant = 1_764_201_600_000_001;
    let batches = connection
        .query_arrow(
            "SELECT epoch_us(CAST($period_start AS TIMESTAMPTZ))::VARCHAR",
            &[SqlValue::TimestampTz(instant)],
        )
        .unwrap();
    let found = arrow::array::cast::as_string_array(batches[0].column(0));
    assert_eq!(found.value(0), instant.to_string());
}

#[test]
fn mg_req_0604_refusal_is_not_a_failure() {
    let mut connection = connection();
    let refused = connection.describe("SELECT nope FROM raw.erp__orders", &[]);
    let Err(PortError::Refused(message)) = refused else {
        panic!("a binder error is a refusal: {refused:?}");
    };
    assert!(message.starts_with("Binder Error: "), "{message}");
    assert!(!message.contains("DESCRIBE"), "{message}");
    for (sql, kind) in [
        ("SELEC 1", "Parser Error: "),
        ("SELECT * FROM missing", "Catalog Error: "),
    ] {
        let refused = connection.describe(sql, &[]);
        assert!(
            matches!(&refused, Err(PortError::Refused(message)) if message.starts_with(kind)),
            "{refused:?}"
        );
    }
}

#[test]
fn create_empty_table_writes_only_column_types() {
    let mut connection = connection();
    for data_type in [
        "INTEGER); DROP TABLE x; --",
        "STRUCT(a INTEGER)",
        "DECIMAL(18,x)",
    ] {
        assert_eq!(
            connection.create_empty_table(None, "t", &[column("a", data_type)]),
            Err(PortError::UnsupportedType(data_type.to_owned()))
        );
    }
    let types = [
        column("a", "DECIMAL(18,3)"),
        column("b \"quoted\"", "TIMESTAMP WITH TIME ZONE"),
    ];
    connection.create_empty_table(None, "t", &types).unwrap();
    assert_eq!(connection.describe("SELECT * FROM t", &[]).unwrap(), types);
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
