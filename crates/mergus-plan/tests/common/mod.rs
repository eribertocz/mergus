//! Models and requests shared by the tests.

#![cfg(test)]
#![allow(dead_code, reason = "each test file uses a part of the helpers")]

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use mergus_plan::{Plan, PlanModel, PlanRequest, plan};
use serde_json::Value;

/// A model `name` whose code is `select from <inputs>` and that reads `inputs`.
pub fn model(name: &str, inputs: &[&str]) -> PlanModel {
    PlanModel {
        name: name.to_owned(),
        model_id: format!(
            "mdl_{:a<16}",
            name.replace('_', "").get(..10).unwrap_or(name)
        ),
        code: format!("SELECT * FROM {}\n", inputs.join(", ")),
        config: BTreeMap::from([("refresh".to_owned(), Value::from("full"))]),
        inputs: inputs.iter().map(|input| (*input).to_owned()).collect(),
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
