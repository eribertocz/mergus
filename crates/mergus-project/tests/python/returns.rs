//! The return decides the engine (MG-REQ-0613).

use mergus_project::StepEngine;

use super::{problems, read, summary};

#[test]
fn mg_req_0613_return_kind_selects_engine() {
    let model = read("sales_by_segment").unwrap();
    let engines: Vec<(&str, &StepEngine)> = model
        .steps
        .iter()
        .map(|step| (step.name.as_str(), &step.engine))
        .collect();
    let joined =
        "\n        SELECT f.*, r.rate FROM fact_sales f JOIN erp__rates r USING (date)\n    ";
    let result = "SELECT segment, SUM(amount_usd) AS total_usd FROM in_usd GROUP BY ALL";
    assert_eq!(
        engines,
        [
            (
                "joined",
                &StepEngine::DuckDb {
                    sql: vec![joined.to_owned()]
                }
            ),
            (
                "sales_by_segment",
                &StepEngine::DuckDb {
                    sql: vec![result.to_owned()]
                }
            ),
            ("in_usd", &StepEngine::Polars),
        ]
    );
    assert_invalid_returns();
}

/// Checks the problems of the fixture `returns` (MG-REQ-0613.2).
fn assert_invalid_returns() {
    let fix = "return SQL as one string literal, or a Polars LazyFrame";
    let invalid = |line: usize, step: &str, what: &str| {
        (
            "MG-0619",
            Some(line),
            format!("`models/returns.py` line {line}: step `{step}` returns {what}; {fix}"),
        )
    };
    // `literal` (adjacent literals) and `frame` (a call) are valid, as is the result.
    assert_eq!(
        summary(&problems("returns")),
        [
            invalid(15, "fstring", "an f-string"),
            invalid(19, "concat", "a binary operation"),
            invalid(23, "percent", "a binary operation"),
            invalid(28, "bare", "a bare variable"),
            invalid(32, "formatted", "`.format(...)` of a string literal"),
            invalid(36, "nothing", "`None`"),
            (
                "MG-0619",
                Some(39),
                format!("`models/returns.py` line 39: step `no_return` has no `return`; {fix}"),
            ),
            (
                "MG-0619",
                Some(46),
                "`models/returns.py` line 46: step `mixed` returns both SQL and a Polars \
                 expression; every return of a step has one kind"
                    .to_owned(),
            ),
        ]
    );
}
