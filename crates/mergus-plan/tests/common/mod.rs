//! Models and requests shared by the tests.

#![cfg(test)]
#![allow(dead_code, reason = "each test file uses a part of the helpers")]

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use mergus_plan::{Plan, PlanModel, PlanRequest, plan};
use serde_json::Value;

/// A model `name` that reads `inputs`, whose code names them.
pub fn model(name: &str, inputs: &[&str]) -> PlanModel {
    PlanModel {
        name: name.to_owned(),
        model_id: format!(
            "mdl_{:a<16}",
            name.replace('_', "").get(..10).unwrap_or(name)
        ),
        code: format!("model {name} reads {}\n", inputs.join(", ")),
        config: BTreeMap::from([("refresh".to_owned(), Value::from("full"))]),
        inputs: inputs.iter().map(|input| (*input).to_owned()).collect(),
        published: false,
        steps: Vec::new(),
    }
}

/// The names `names` as a set.
pub fn names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// A request for `selected` in the environment `dev`.
pub fn request(selected: &[&str]) -> PlanRequest {
    PlanRequest::new("0.1.0", "dev", names(selected))
}

/// `request` with `max_parallel` set to `max`.
pub fn with_max(mut request: PlanRequest, max: u32) -> PlanRequest {
    request.max_parallel = NonZeroU32::new(max).unwrap();
    request
}

/// The plan of `models` for `request`.
pub fn planned(models: &[PlanModel], request: &PlanRequest) -> Plan {
    plan(models, request).unwrap()
}

/// The project `raw.erp__orders -> stg_orders -> fact_sales <- dim_customer`.
pub fn shop() -> Vec<PlanModel> {
    vec![
        model("stg_orders", &["raw.erp__orders"]),
        model("dim_customer", &[]),
        model("fact_sales", &["dim_customer", "stg_orders"]),
    ]
}

/// The fingerprint of the step of `name` in `plan`.
pub fn fingerprint_of(plan: &Plan, name: &str) -> String {
    plan.document()
        .steps
        .iter()
        .find(|step| step.model == name)
        .map(|step| step.fingerprint.clone())
        .unwrap()
}

/// The `(model, parallel_group)` of each step of `plan`, in order.
pub fn groups(plan: &Plan) -> Vec<(String, i64)> {
    plan.document()
        .steps
        .iter()
        .map(|step| (step.model.clone(), step.parallel_group))
        .collect()
}

/// A step `name` on `engine` that outputs `columns`, each `<name> <DuckDB type>`, and reads
/// `reads` as `(step, column)`.
pub fn step(
    name: &str,
    engine: mergus_plan::Engine,
    columns: &[&str],
    reads: &[(&str, &str)],
) -> mergus_plan::ModelStep {
    mergus_plan::ModelStep {
        name: name.to_owned(),
        engine,
        columns: columns
            .iter()
            .map(|column| {
                let (name, data_type) = column.split_once(' ').unwrap();
                mergus_plan::OutputColumn {
                    name: name.to_owned(),
                    data_type: data_type.to_owned(),
                }
            })
            .collect(),
        reads: reads
            .iter()
            .map(|(step, column)| mergus_plan::ColumnRead {
                step: (*step).to_owned(),
                column: (*column).to_owned(),
            })
            .collect(),
        detected: Vec::new(),
    }
}

/// The model `name`, with no input, made of `steps`.
pub fn with_steps(name: &str, steps: Vec<mergus_plan::ModelStep>) -> PlanModel {
    let mut made = model(name, &[]);
    made.steps = steps;
    made
}
