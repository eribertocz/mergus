//! Models and helpers shared by the refresh tests.

#![cfg(test)]

use mergus_contracts::generated::mergus_plan_v1::Step;
use mergus_plan::{Instant, PlanError, PlanModel, Range, RefreshError, plan};
use serde_json::{Value, json};

use super::{model, request};

/// `model` with the options `options` set.
pub fn configured(name: &str, inputs: &[&str], options: &[(&str, Value)]) -> PlanModel {
    let mut made = model(name, inputs);
    for (option, value) in options {
        made.config.insert((*option).to_owned(), value.clone());
    }
    made
}

/// An incremental model `fact_sales` by day of `sold_at`, reading `stg_orders` and
/// `dim_customer`.
pub fn daily() -> PlanModel {
    configured(
        "fact_sales",
        &["dim_customer", "stg_orders"],
        &[
            ("refresh", json!("incremental")),
            ("partition_by", json!("sold_at:day")),
        ],
    )
}

/// Midnight UTC of `year-month-day`.
pub fn day(year: i32, month: u32, day: u32) -> Instant {
    Instant::from_date(year, month, day).unwrap()
}

/// The range `[start, end)`.
pub fn range(start: Instant, end: Instant) -> Range {
    Range::new(start, end).unwrap()
}

/// The step of `name` in `plan`.
pub fn step_of<'a>(plan: &'a mergus_plan::Plan, name: &str) -> &'a Step {
    plan.document()
        .steps
        .iter()
        .find(|step| step.model == name)
        .unwrap()
}

/// The `(start, end)` of each period of `step`.
pub fn periods(step: &Step) -> Vec<(String, String)> {
    step.periods
        .as_ref()
        .unwrap()
        .iter()
        .map(|period| (period.start.clone(), period.end.clone()))
        .collect()
}

/// The pair of RFC 3339 texts.
pub fn texts(start: &str, end: &str) -> (String, String) {
    (start.to_owned(), end.to_owned())
}

/// The refresh errors of a failed plan.
pub fn refresh_errors(error: PlanError) -> Vec<RefreshError> {
    match error {
        PlanError::Refresh(errors) => errors,
        other => panic!("not a refresh error: {other}"),
    }
}

/// The error of planning every model of `models`.
pub fn plan_of(models: &[PlanModel]) -> PlanError {
    let all: Vec<&str> = models.iter().map(|model| model.name.as_str()).collect();
    plan(models, &request(&all)).unwrap_err()
}
