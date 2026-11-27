//! The plan: MG-REQ-0005, MG-REQ-0713 to MG-REQ-0717 and MG-REQ-0742 to MG-REQ-0745.

mod common;

use std::collections::BTreeSet;

use common::{fingerprint_of, groups, model, names, planned, request, shop, with_max};
use mergus_contracts::Contract;
use mergus_contracts::generated::mergus_plan_v1::MergusPlan;
use mergus_plan::{DEFAULT_MAX_PARALLEL, PlanError, plan};
use serde_json::{Value, json};

/// Every model of [`shop`], selected.
const ALL: [&str; 3] = ["dim_customer", "fact_sales", "stg_orders"];

/// Five independent models.
fn independent() -> Vec<mergus_plan::PlanModel> {
    ["e", "d", "c", "b", "a"]
        .iter()
        .map(|name| model(name, &[]))
        .collect()
}

#[test]
fn mg_req_0005_plan_is_byte_identical() {
    let mut reversed = shop();
    reversed.reverse();
    let first = planned(&shop(), &request(&ALL));
    let second = planned(
        &reversed,
        &request(&["stg_orders", "fact_sales", "dim_customer"]),
    );
    assert_eq!(first.bytes(), second.bytes());
    assert_eq!(first.plan_id(), second.plan_id());
}

#[test]
fn mg_req_0715_plan_byte_identical() {
    let plans: Vec<Vec<u8>> = (0..3)
        .map(|_| planned(&shop(), &request(&["fact_sales"])).bytes())
        .collect();
    assert!(plans.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn mg_req_0713_plan_records_decisions_and_reasons() {
    let models = shop();
    let mut request = request(&ALL);
    let first = planned(&models, &request);
    request
        .available
        .insert(fingerprint_of(&first, "dim_customer"));
    let plan = planned(&models, &request);
    let decisions: Vec<(String, Value, Value, Value)> = plan.valid().value()["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| {
            let text = step["model"].as_str().unwrap().to_owned();
            (
                text,
                step["action"].clone(),
                step["reason"].clone(),
                step["refresh"].clone(),
            )
        })
        .collect();
    assert_eq!(
        decisions,
        [
            (
                "dim_customer".to_owned(),
                json!("skip"),
                json!("result_available"),
                json!("full")
            ),
            (
                "stg_orders".to_owned(),
                json!("compute"),
                json!("no_result"),
                json!("full")
            ),
            (
                "fact_sales".to_owned(),
                json!("compute"),
                json!("no_result"),
                json!("full")
            ),
        ]
    );
}

#[test]
fn mg_req_0714_plan_is_hashed_artifact() {
    let plan = planned(&shop(), &request(&ALL));
    let read: MergusPlan = mergus_contracts::read(plan.valid().value()).unwrap();
    assert_eq!(&read, plan.document());
    assert_eq!(plan.plan_id().len(), 64);
    assert!(
        plan.plan_id()
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    );
    assert_eq!(plan.bytes(), plan.valid().hash_input());
    let other = planned(&shop(), &request(&["fact_sales"]));
    assert_ne!(plan.plan_id(), other.plan_id());
}

#[test]
fn mg_req_0716_parallelism_respects_budget() {
    let serial = planned(
        &independent(),
        &with_max(request(&["a", "b", "c", "d", "e"]), 1),
    );
    let budget = planned(
        &independent(),
        &with_max(request(&["a", "b", "c", "d", "e"]), 2),
    );
    let group_numbers = |plan| groups(plan).into_iter().map(|(_, g)| g).collect::<Vec<_>>();
    assert_eq!(group_numbers(&serial), [1, 2, 3, 4, 5]);
    assert_eq!(group_numbers(&budget), [1, 1, 2, 2, 3]);
}

#[test]
fn mg_req_0717_stale_upstream_reported() {
    let models = shop();
    let plan = planned(&models, &request(&["fact_sales"]));
    let stale = &plan.valid().value()["stale_upstream"];
    assert_eq!(
        *stale,
        json!([
            {"model": "dim_customer", "include": "+fact_sales"},
            {"model": "stg_orders", "include": "+fact_sales"},
        ])
    );
    let every = planned(&models, &request(&ALL));
    let mut fresh = request(&["fact_sales"]);
    fresh.available = ["dim_customer", "stg_orders"]
        .iter()
        .map(|name| fingerprint_of(&every, name))
        .collect();
    assert_eq!(
        planned(&models, &fresh).valid().value()["stale_upstream"],
        json!([])
    );
}

/// The member names of a JSON object, sorted.
fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .map(|members| members.keys().map(String::as_str).collect())
        .unwrap_or_default();
    keys.sort_unstable();
    keys
}

/// The fields of a plan (MG-REQ-0742), sorted.
const PLAN_FIELDS: [&str; 9] = [
    "contract",
    "contract_version",
    "environment",
    "max_parallel",
    "mergus_version",
    "plan_id",
    "selected",
    "stale_upstream",
    "steps",
];

/// The fields of a step (MG-REQ-0742), sorted.
const STEP_FIELDS: [&str; 8] = [
    "action",
    "fingerprint",
    "inputs",
    "model",
    "model_id",
    "parallel_group",
    "reason",
    "refresh",
];

#[test]
fn mg_req_0742_plan_contract_fields() {
    assert_eq!(
        (MergusPlan::NAME, MergusPlan::SEMANTIC_VERSION),
        ("mergus_plan", "1.1.0")
    );
    // The generated type includes `schemas/mergus_plan/v1/schema.json`.
    let schema: Value = serde_json::from_str(MergusPlan::SCHEMA).unwrap();
    assert_eq!(
        schema["$id"],
        json!("https://mergus.dev/schemas/mergus_plan/v1/schema.json")
    );
    let plan = planned(&shop(), &request(&["stg_orders", "fact_sales"]));
    let value = plan.valid().value();
    assert_eq!(keys(value), PLAN_FIELDS);
    assert_eq!(keys(&value["steps"][0]), STEP_FIELDS);
    assert_eq!(value["contract"], json!("mergus_plan"));
    assert_eq!(value["contract_version"], json!(1));
    assert_eq!(value["environment"], json!("dev"));
    assert_eq!(value["mergus_version"], json!("0.1.0"));
    assert_eq!(value["selected"], json!(["fact_sales", "stg_orders"]));
    assert_eq!(
        value["steps"][1]["inputs"],
        json!(["dim_customer", "stg_orders"])
    );
    assert_eq!(value["steps"][1]["model_id"], json!(shop()[2].model_id));
    assert_eq!(keys(&value["stale_upstream"][0]), ["include", "model"]);
}

#[test]
fn mg_req_0744_decisions_results_and_groups() {
    // MG-REQ-0744.1: results and `max_parallel` come from the caller, 4 by default.
    assert_eq!(DEFAULT_MAX_PARALLEL.get(), 4);
    let default = planned(&independent(), &request(&["a", "b", "c", "d", "e"]));
    assert_eq!(default.document().max_parallel, 4);
    assert_eq!(
        groups(&default).iter().map(|(_, g)| *g).collect::<Vec<_>>(),
        [1, 1, 1, 1, 2]
    );
    // MG-REQ-0744.2: `skip` with `result_available` only for an available fingerprint.
    let mut request_with = request(&["a"]);
    request_with.available.insert("0".repeat(64));
    let planned_with = planned(&independent(), &request_with);
    let step = &planned_with.document().steps[0];
    assert_eq!(serde_json::to_value(step.action).unwrap(), json!("compute"));
    assert_eq!(
        serde_json::to_value(step.reason).unwrap(),
        json!("no_result")
    );
    // MG-REQ-0744.3: by depth, then name; numbered from 1; at most `max_parallel` each.
    let models = vec![
        model("z_root", &[]),
        model("a_root", &[]),
        model("m_mid", &["z_root"]),
        model("b_mid", &["a_root"]),
        model("c_mid", &["a_root"]),
        model("top", &["b_mid", "m_mid"]),
    ];
    let selected = ["a_root", "b_mid", "c_mid", "m_mid", "top", "z_root"];
    let plan = planned(&models, &with_max(request(&selected), 2));
    let expected = [
        ("a_root", 1),
        ("z_root", 1),
        ("b_mid", 2),
        ("c_mid", 2),
        ("m_mid", 3),
        ("top", 4),
    ];
    let expected: Vec<(String, i64)> = expected
        .iter()
        .map(|(m, g)| ((*m).to_owned(), *g))
        .collect();
    assert_eq!(groups(&plan), expected);
}

#[test]
fn mg_req_0745_stale_upstream_listed() {
    let models = vec![
        model("raw_a", &[]),
        model("stg", &["raw_a"]),
        model("y_report", &["stg"]),
        model("b_report", &["stg"]),
        model("x_other", &[]),
    ];
    let plan = planned(&models, &request(&["b_report", "x_other", "y_report"]));
    assert_eq!(
        plan.valid().value()["stale_upstream"],
        json!([
            {"model": "raw_a", "include": "+b_report"},
            {"model": "stg", "include": "+b_report"},
        ])
    );
    // A selected model is a step, never stale upstream.
    let selected: BTreeSet<String> = names(&["stg", "y_report"]);
    let mut with_stg = request(&[]);
    with_stg.selected = selected;
    let listed = planned(&models, &with_stg).valid().value()["stale_upstream"].clone();
    assert_eq!(listed, json!([{"model": "raw_a", "include": "+stg"}]));
}

#[test]
fn unknown_selection_and_cycles_are_internal_errors() {
    assert!(matches!(
        plan(&shop(), &request(&["ghost"])),
        Err(PlanError::UnknownSelected(name)) if name == "ghost"
    ));
    let cycle = [model("a", &["b"]), model("b", &["a"])];
    assert!(matches!(
        plan(&cycle, &request(&["a"])),
        Err(PlanError::Cycle(models)) if models == ["a", "b"]
    ));
}
