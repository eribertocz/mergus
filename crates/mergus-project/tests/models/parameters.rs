//! MG-REQ-0603: run parameters are named, of fixed types, and bound, never substituted.

use mergus_ports::{Database, DatabaseLocation, SqlValue};
use mergus_project::{LayoutError, RunParameters, read_sql_step};
use mergus_sim::{DatabaseFaults, SimDatabase};

use super::{PATH, known, parser, sql};

/// The `(code, line, column)` of each problem of the fixture `name`, read as a step.
fn problems(name: &str) -> Vec<(&'static str, Option<usize>, Option<usize>)> {
    match read_sql_step(PATH, sql(name).as_bytes(), &known(), &parser()) {
        Err(LayoutError::Invalid(problems)) => problems
            .iter()
            .map(|problem| (problem.code(), problem.line(), problem.column()))
            .collect(),
        other => vec![(
            "unexpected",
            None,
            other.ok().map(|step| step.parameters.len()),
        )],
    }
}

#[test]
fn mg_req_0603_parameters_bound_not_substituted() {
    let text = sql("parameters");
    let step = read_sql_step(PATH, text.as_bytes(), &known(), &parser()).unwrap();
    // The text is the file, unchanged; the parameters are named by the parser, in the order
    // DuckDB numbers them.
    assert_eq!(step.text, text);
    assert_eq!(step.parameters, ["run_date", "period_start", "period_end"]);

    // Each run parameter has its fixed type: `$run_date` a DATE, the periods TIMESTAMPTZ.
    let run = RunParameters {
        run_date: 20_419,
        period_start: 1_764_201_600_000_000,
        period_end: 1_764_288_000_000_000,
    };
    let bound = step.bind(&run);
    assert_eq!(
        bound,
        [
            SqlValue::Date(20_419),
            SqlValue::TimestampTz(1_764_201_600_000_000),
            SqlValue::TimestampTz(1_764_288_000_000_000),
        ]
    );

    // At execution the database receives the text with `$run_date` and the values apart.
    let database = SimDatabase::new(0, DatabaseFaults::default());
    database.record_query(text, bound.clone(), Vec::new());
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    connection.query_arrow(&step.text, &bound).unwrap();
    let queried = database.queried();
    assert_eq!(queried, [(text.to_owned(), bound)]);
    for name in ["$run_date", "$period_start", "$period_end"] {
        assert!(queried[0].0.contains(name), "{}", queried[0].0);
    }
    assert!(!queried[0].0.contains("20419"), "{}", queried[0].0);
}

#[test]
fn mg_req_0603_only_the_run_parameters_by_name() {
    // A name other than `run_date`, `period_start` and `period_end` is MG-0602, at the
    // parameter.
    assert_eq!(
        problems("unknown_parameter"),
        [("MG-0602", Some(3), Some(16))]
    );
    // A positional parameter, `?` or `$1`, is MG-0603, at the parameter.
    assert_eq!(problems("positional"), [("MG-0603", Some(3), Some(18))]);
    assert_eq!(problems("numbered"), [("MG-0603", Some(3), Some(18))]);
}

#[test]
fn mg_req_0603_parameter_errors_name_the_parameter() {
    let message = |name: &str| match read_sql_step(PATH, sql(name).as_bytes(), &known(), &parser())
    {
        Err(LayoutError::Invalid(problems)) => problems[0].to_string(),
        other => format!("{other:?}"),
    };
    let unknown = message("unknown_parameter");
    assert!(unknown.contains("`$min_amount`"), "{unknown}");
    assert!(unknown.contains("`$run_date`"), "{unknown}");
    assert!(message("positional").contains("`?`"));
    assert!(message("numbered").contains("`$1`"));
}
