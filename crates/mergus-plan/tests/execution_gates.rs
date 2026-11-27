//! The `Type passport` and `Determinism` gates: MG-REQ-0634 and MG-REQ-0637.

#![cfg(test)]

mod common;

use std::collections::BTreeMap;

use common::{step, with_steps};
use mergus_plan::{
    Detected, Engine, Gate, GateCheck, GateInput, Operation, PlanModel, detect_sql, determinism,
    evaluate, type_passport,
};
use serde_json::Value;

/// A DuckDB step `load` that outputs `columns`, read whole by a Polars step `score`.
fn into_polars(columns: &[&str]) -> PlanModel {
    let names: Vec<&str> = columns
        .iter()
        .map(|column| column.split_once(' ').unwrap().0)
        .collect();
    let reads: Vec<(&str, &str)> = names.iter().map(|name| ("load", *name)).collect();
    with_steps(
        "stg_orders",
        vec![
            step("load", Engine::Duckdb, columns, &[]),
            step("score", Engine::Polars, &["id INTEGER"], &reads),
        ],
    )
}

/// The input of stage 4 with `gate` checked with `errors`, every other gate passing.
fn gate_input(gate: Gate, errors: Vec<mergus_plan::GateError>) -> GateInput {
    let mut checks: BTreeMap<Gate, GateCheck> = Gate::ALL
        .into_iter()
        .map(|gate| (gate, GateCheck::Checked(Vec::new())))
        .collect();
    checks.insert(gate, GateCheck::Checked(errors));
    GateInput {
        checks,
        ..GateInput::default()
    }
}

#[test]
fn mg_req_0634_lossy_boundary_blocks() {
    let model = into_polars(&[
        "id INTEGER",
        "big HUGEINT",
        "wait INTERVAL",
        "price DECIMAL(18,3)",
        "tags VARCHAR[]",
        "nested STRUCT(a UHUGEINT, b VARCHAR)",
        "odd GEOMETRY",
    ]);
    let errors = type_passport(&[model]);
    let found: Vec<(&str, Option<&str>, Option<&str>)> = errors
        .iter()
        .map(|error| (error.code, error.model.as_deref(), error.step.as_deref()))
        .collect();
    assert_eq!(
        found,
        vec![("MG-0702", Some("stg_orders"), Some("load")); 4]
    );
    let messages: Vec<&str> = errors.iter().map(|error| error.message.as_str()).collect();
    for (at, (column, data_type)) in [
        ("big", "HUGEINT"),
        ("wait", "INTERVAL"),
        ("nested", "STRUCT(a UHUGEINT, b VARCHAR)"),
        ("odd", "GEOMETRY"),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(messages[at].contains(&format!("`{column}` of type `{data_type}`")));
    }
    let report = evaluate(&gate_input(Gate::TypePassport, errors)).unwrap();
    assert!(report.clear().is_err());
}

#[test]
fn mg_req_0634_lossless_boundaries_pass() {
    let into = into_polars(&[
        "id INTEGER",
        "price DECIMAL(38,2)",
        "day TIMESTAMP WITH TIME ZONE",
        "tags MAP(VARCHAR, INTEGER[3])",
        "kind ENUM('a', 'b')",
    ]);
    assert!(type_passport(&[into]).is_empty());
    let back = with_steps(
        "back",
        vec![
            step("score", Engine::Polars, &["big HUGEINT"], &[]),
            step(
                "load",
                Engine::Duckdb,
                &["big HUGEINT"],
                &[("score", "big")],
            ),
        ],
    );
    assert!(type_passport(&[back]).is_empty());
    let digits = into_polars(&["wide DECIMAL(39,2)"]);
    assert_eq!(type_passport(&[digits]).len(), 1);
}

/// The SQL fixtures of `sql/`: each text and its tree, recorded from `json_serialize_sql` of
/// the bundled DuckDB through the real `SqlParser` port.
const FIXTURES: [(&str, &str, &str); 5] = [
    (
        "nondeterministic",
        include_str!("sql/nondeterministic.sql"),
        include_str!("sql/nondeterministic.json"),
    ),
    (
        "declared",
        include_str!("sql/declared.sql"),
        include_str!("sql/declared.json"),
    ),
    (
        "window",
        include_str!("sql/window.sql"),
        include_str!("sql/window.json"),
    ),
    (
        "offset",
        include_str!("sql/offset.sql"),
        include_str!("sql/offset.json"),
    ),
    (
        "deterministic",
        include_str!("sql/deterministic.sql"),
        include_str!("sql/deterministic.json"),
    ),
];

/// The text and the recorded tree of the fixture `name` of `sql/`.
fn fixture(name: &str) -> (String, Value) {
    let (_, text, tree) = FIXTURES
        .iter()
        .find(|(known, _, _)| *known == name)
        .unwrap();
    ((*text).to_owned(), serde_json::from_str(tree).unwrap())
}

/// The operations, lines and columns found in the fixture `name`.
fn detected(name: &str) -> Vec<(Operation, usize, usize)> {
    let (text, tree) = fixture(name);
    detect_sql(&text, &tree)
        .into_iter()
        .map(|found| (found.operation, found.line, found.column))
        .collect()
}

/// A published model whose step `query` found `found`, declaring `declared`.
fn published(found: Vec<Detected>, declared: &[&str]) -> PlanModel {
    let mut model = with_steps(
        "daily_sales",
        vec![step("query", Engine::Duckdb, &["id INTEGER"], &[])],
    );
    model.published = true;
    model.steps[0].detected = found;
    model.config.insert(
        "nondeterministic".to_owned(),
        Value::from(declared.to_vec()),
    );
    model
}

#[test]
fn mg_req_0637_nondeterminism_flagged() {
    assert_eq!(
        detected("nondeterministic"),
        vec![
            (Operation::Random, 1, 12),
            (Operation::Now, 1, 27),
            (Operation::LimitWithoutOrder, 2, 35),
        ]
    );
    assert_eq!(detected("declared"), vec![(Operation::AnyValue, 1, 12)]);
    assert!(detected("deterministic").is_empty());

    let (text, tree) = fixture("nondeterministic");
    let errors = determinism(&[published(detect_sql(&text, &tree), &["now"])]);
    let found: Vec<(&str, Option<&str>, Option<&str>)> = errors
        .iter()
        .map(|error| (error.code, error.model.as_deref(), error.step.as_deref()))
        .collect();
    assert_eq!(
        found,
        vec![("MG-0703", Some("daily_sales"), Some("query")); 2]
    );
    assert!(
        errors[0]
            .message
            .starts_with("`random` at line 1, column 12")
    );
    assert!(
        errors[1]
            .message
            .starts_with("`limit_without_order` at line 2, column 35")
    );
    let report = evaluate(&gate_input(Gate::Determinism, errors)).unwrap();
    assert!(report.clear().is_err());
}

#[test]
fn mg_req_0637_declared_or_unpublished_passes() {
    let (text, tree) = fixture("declared");
    let found = detect_sql(&text, &tree);
    assert!(determinism(&[published(found.clone(), &["any_value"])]).is_empty());
    let mut intermediate = published(found, &[]);
    intermediate.published = false;
    assert!(determinism(&[intermediate]).is_empty());
}

#[test]
fn mg_req_0637_window_calls_flagged() {
    assert_eq!(detected("window"), vec![(Operation::AnyValue, 1, 12)]);
}

#[test]
fn mg_req_0637_offset_without_limit_not_flagged() {
    assert!(detected("offset").is_empty());
}

#[test]
fn mg_req_0634_same_name_from_two_steps_both_checked() {
    let model = with_steps(
        "stg_orders",
        vec![
            step("a", Engine::Duckdb, &["x BIGINT"], &[]),
            step("b", Engine::Duckdb, &["x HUGEINT"], &[]),
            step(
                "score",
                Engine::Polars,
                &["y DOUBLE"],
                &[("a", "x"), ("b", "x")],
            ),
        ],
    );
    let errors = type_passport(&[model]);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].step.as_deref(), Some("b"));
    assert!(errors[0].message.contains("`x` of type `HUGEINT`"));
}
