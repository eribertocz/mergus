//! Storage, backfill and history options: MG-REQ-0656, MG-REQ-0657 and MG-REQ-0661.

#![cfg(test)]

mod common;

use common::refresh::{configured, day, periods, plan_of, range, refresh_errors, step_of, texts};
use common::{model, names, planned, request};
use mergus_contracts::generated::mergus_plan_v1::{StepAction, StepReason, StepRefresh};
use mergus_plan::{Backfill, By, Deletes, Refresh, RefreshError, Storage, plan, refresh};
use serde_json::json;

#[test]
fn mg_req_0656_intermediates_not_stored_by_default() {
    let mut published = model("fact_sales", &["int_orders", "int_returns"]);
    published.published = true;
    let on_the_fly = model("int_orders", &[]);
    let materialized = configured("int_returns", &[], &[("materialize", json!(true))]);
    let plan = planned(
        &[published, on_the_fly, materialized],
        &request(&["fact_sales", "int_orders", "int_returns"]),
    );
    assert_eq!(plan.storage("fact_sales"), Some(Storage::Published));
    assert_eq!(plan.storage("int_orders"), Some(Storage::NotStored));
    assert_eq!(plan.storage("int_returns"), Some(Storage::Internal));
    assert_eq!(plan.storage("unknown"), None);
    assert_eq!(plan.stored(), names(&["fact_sales", "int_returns"]));
}

#[test]
fn mg_req_0657_backfill_recomputes_range() {
    let weekly = configured(
        "fact_sales",
        &[],
        &[
            ("refresh", json!("incremental")),
            ("partition_by", json!("sold_at:week")),
        ],
    );
    let mut request = request(&["fact_sales"]);
    request.affected.insert(
        "fact_sales".to_owned(),
        vec![range(day(2026, 6, 1), day(2026, 6, 8))],
    );
    request.backfill.insert(
        "fact_sales".to_owned(),
        Backfill {
            from: day(2026, 3, 4),
            to: day(2026, 3, 18),
        },
    );
    let first = planned(std::slice::from_ref(&weekly), &request);
    let step = step_of(&first, "fact_sales");
    assert_eq!(
        periods(step),
        vec![
            texts("2026-03-02T00:00:00Z", "2026-03-23T00:00:00Z"),
            texts("2026-06-01T00:00:00Z", "2026-06-08T00:00:00Z"),
        ]
    );

    // Whatever the changes: an available result is still recomputed.
    request.affected.clear();
    request.available.insert(step.fingerprint.clone());
    let again = planned(std::slice::from_ref(&weekly), &request);
    let step = step_of(&again, "fact_sales");
    assert_eq!(step.action, StepAction::Compute);
    assert_eq!(
        periods(step),
        vec![texts("2026-03-02T00:00:00Z", "2026-03-23T00:00:00Z")]
    );
}

#[test]
fn mg_req_0657_model_without_partition_rejected() {
    let mut request = request(&["fact_sales"]);
    request.backfill.insert(
        "fact_sales".to_owned(),
        Backfill {
            from: day(2026, 3, 4),
            to: day(2026, 3, 18),
        },
    );
    let by_key = configured(
        "fact_sales",
        &[],
        &[
            ("refresh", json!("incremental")),
            ("key", json!("order_id")),
        ],
    );
    let errors = refresh_errors(plan(&[by_key], &request).unwrap_err());
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), "MG-0706");
    assert_eq!(
        errors[0].to_string(),
        "model `fact_sales` cannot be backfilled: it has no `partition_by`; give it \
         `partition_by = \"<column>:<grain>\"`"
    );
}

#[test]
fn mg_req_0657_full_and_history_backfilled_whole() {
    let mut request = request(&["fact_sales"]);
    request.backfill.insert(
        "fact_sales".to_owned(),
        Backfill {
            from: day(2026, 3, 4),
            to: day(2026, 3, 18),
        },
    );
    let full = configured(
        "fact_sales",
        &[],
        &[("partition_by", json!("sold_at:month"))],
    );
    let first = planned(std::slice::from_ref(&full), &request);
    request
        .available
        .insert(step_of(&first, "fact_sales").fingerprint.clone());
    let backfilled = planned(std::slice::from_ref(&full), &request);
    let step = step_of(&backfilled, "fact_sales");
    assert_eq!(step.action, StepAction::Compute);
    assert_eq!(step.reason, StepReason::NoResult);
    assert_eq!(step.refresh, StepRefresh::Full);
    assert_eq!(step.periods, None);

    let history = configured(
        "fact_sales",
        &[],
        &[
            ("refresh", json!("history")),
            ("key", json!("order_id")),
            ("by", json!("updated_at")),
            ("partition_by", json!("sold_at:month")),
        ],
    );
    let errors = refresh_errors(plan(std::slice::from_ref(&history), &request).unwrap_err());
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), "MG-0706");
    assert_eq!(
        errors[0].to_string(),
        "history model `fact_sales` cannot be backfilled without confirming the rebuild of its \
         history; a backfill rebuilds every version it holds"
    );
    request.history_rebuild_confirmed = true;
    let rebuilt = planned(&[history], &request);
    let step = step_of(&rebuilt, "fact_sales");
    assert_eq!(step.action, StepAction::Compute);
    assert_eq!(step.periods, None);
}

#[test]
fn mg_req_0661_history_requires_key_and_by() {
    let full = configured(
        "dim_customer",
        &[],
        &[
            ("refresh", json!("history")),
            ("key", json!("customer_id")),
            ("by", json!(["segment", "city"])),
        ],
    );
    assert_eq!(
        refresh(&full).unwrap(),
        Refresh::History {
            key: vec!["customer_id".to_owned()],
            by: By::Columns(vec!["segment".to_owned(), "city".to_owned()]),
            deletes: Deletes::Ignore,
        }
    );
    let mut closing = full.clone();
    closing.config.insert("by".to_owned(), json!("updated_at"));
    closing.config.insert("deletes".to_owned(), json!("close"));
    assert_eq!(
        refresh(&closing).unwrap(),
        Refresh::History {
            key: vec!["customer_id".to_owned()],
            by: By::Column("updated_at".to_owned()),
            deletes: Deletes::Close,
        }
    );
    let plan = planned(&[full], &request(&["dim_customer"]));
    assert_eq!(step_of(&plan, "dim_customer").refresh, StepRefresh::Full);
    assert_eq!(step_of(&plan, "dim_customer").incremental, None);

    let bare = configured("dim_customer", &[], &[("refresh", json!("history"))]);
    let error = refresh_errors(refresh(&bare).unwrap_err()).remove(0);
    assert_eq!(error.code(), "MG-0705");
    assert_eq!(
        error.to_string(),
        "history model `dim_customer` does not declare `key` and `by`; declare the business \
         key and the column or columns that detect a change"
    );
    let errors = refresh_errors(plan_of(&[
        bare,
        configured("fact_sales", &[], &[("refresh", json!("incremental"))]),
    ]));
    let codes: Vec<&str> = errors.iter().map(RefreshError::code).collect();
    assert_eq!(codes, ["MG-0705", "MG-0704"]);
}
