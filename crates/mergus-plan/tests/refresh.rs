//! Refresh: MG-REQ-0651 to MG-REQ-0654.

#![cfg(test)]

mod common;

use common::refresh::{configured, daily, day, periods, range, refresh_errors, step_of, texts};
use common::{model, planned, request};
use mergus_contracts::generated::mergus_plan_v1::{IncrementalMode, StepAction, StepRefresh};
use mergus_plan::{Grain, Instant, Range, Refresh, plan, refresh};
use serde_json::json;

#[test]
fn mg_req_0651_full_refresh_default() {
    let mut plain = model("dim_customer", &[]);
    plain.config.remove("refresh");
    assert_eq!(refresh(&plain).unwrap(), Refresh::Full);
    let first = planned(std::slice::from_ref(&plain), &request(&["dim_customer"]));
    let step = step_of(&first, "dim_customer");
    assert_eq!(step.refresh, StepRefresh::Full);
    assert_eq!(step.incremental, None);
    assert_eq!(step.periods, None);
    assert!(
        first.valid().value()["steps"][0]
            .get("incremental")
            .is_none()
    );

    let mut available = request(&["dim_customer"]);
    available.available.insert(step.fingerprint.clone());
    let unchanged = planned(std::slice::from_ref(&plain), &available);
    assert_eq!(step_of(&unchanged, "dim_customer").action, StepAction::Skip);
    plain.code.push_str("-- changed\n");
    let changed = planned(&[plain], &available);
    let step = step_of(&changed, "dim_customer");
    assert_eq!(step.action, StepAction::Compute);
    assert_eq!(step.refresh, StepRefresh::Full);
}

#[test]
fn mg_req_0652_only_affected_periods_recomputed() {
    assert_eq!(
        refresh(&daily()).unwrap(),
        Refresh::Period {
            column: "sold_at".to_owned(),
            grain: Grain::Day,
        }
    );
    let models = [
        model("dim_customer", &[]),
        model("stg_orders", &[]),
        daily(),
    ];
    let mut request = request(&["fact_sales"]);
    request.affected.insert(
        "fact_sales".to_owned(),
        vec![
            range(day(2026, 3, 9), day(2026, 3, 10)),
            range(day(2026, 3, 1), day(2026, 3, 2)),
            range(day(2026, 3, 2), day(2026, 3, 3)),
            range(day(2026, 3, 9), day(2026, 3, 10)),
            range(day(2026, 3, 5), day(2026, 3, 6)),
        ],
    );
    let plan = planned(&models, &request);
    let step = step_of(&plan, "fact_sales");
    assert_eq!(step.refresh, StepRefresh::Full);
    assert_eq!(
        step.incremental.as_ref().unwrap().mode,
        IncrementalMode::Period
    );
    assert_eq!(
        periods(step),
        vec![
            texts("2026-03-01T00:00:00Z", "2026-03-03T00:00:00Z"),
            texts("2026-03-05T00:00:00Z", "2026-03-06T00:00:00Z"),
            texts("2026-03-09T00:00:00Z", "2026-03-10T00:00:00Z"),
        ]
    );
    assert_eq!(
        plan.valid().value()["steps"][0]["incremental"],
        json!({"mode": "period"})
    );
}

#[test]
fn mg_req_0652_changed_dimension_recomputes_every_period() {
    let models = [
        model("dim_customer", &[]),
        model("stg_orders", &[]),
        daily(),
    ];
    let mut request = request(&["fact_sales"]);
    request.affected.insert(
        "fact_sales".to_owned(),
        vec![range(day(2026, 3, 9), day(2026, 3, 10))],
    );
    // A dimension changed: every period, given as one range, swallows the others.
    request
        .affected
        .get_mut("fact_sales")
        .unwrap()
        .push(range(day(2025, 1, 1), day(2026, 3, 10)));
    let every = planned(&models, &request);
    assert_eq!(
        periods(step_of(&every, "fact_sales")),
        vec![texts("2025-01-01T00:00:00Z", "2026-03-10T00:00:00Z")]
    );

    // Nothing changed: an empty list.
    request.affected.clear();
    let none = planned(&models, &request);
    assert_eq!(periods(step_of(&none, "fact_sales")), Vec::new());
    assert_eq!(none.valid().value()["steps"][0]["periods"], json!([]));
}

#[test]
fn mg_req_0652_periods_are_whole_utc_grains() {
    let at = Instant::from_unix_seconds(1_773_165_845).unwrap(); // 2026-03-10T18:04:05Z, a Tuesday
    let of = |grain: Grain| {
        let period = grain.period(at);
        (period.start().to_string(), period.end().to_string())
    };
    assert_eq!(
        of(Grain::Day),
        texts("2026-03-10T00:00:00Z", "2026-03-11T00:00:00Z")
    );
    assert_eq!(
        of(Grain::Week),
        texts("2026-03-09T00:00:00Z", "2026-03-16T00:00:00Z")
    );
    assert_eq!(
        of(Grain::Month),
        texts("2026-03-01T00:00:00Z", "2026-04-01T00:00:00Z")
    );
    assert_eq!(
        of(Grain::Quarter),
        texts("2026-01-01T00:00:00Z", "2026-04-01T00:00:00Z")
    );
    assert_eq!(
        of(Grain::Year),
        texts("2026-01-01T00:00:00Z", "2027-01-01T00:00:00Z")
    );
    // A Sunday belongs to the week that started on the Monday before it.
    let sunday = Grain::Week.period(day(2026, 3, 15));
    assert_eq!(sunday.start(), day(2026, 3, 9));
    // A period holds its start, not its end.
    assert_eq!(
        Grain::Day.period(day(2026, 3, 11)).start(),
        day(2026, 3, 11)
    );
    assert_eq!(Instant::from_date(2026, 2, 29), None);
    assert_eq!(Range::new(day(2026, 3, 2), day(2026, 3, 2)), None);
}

#[test]
fn mg_req_0653_incremental_merge_by_key() {
    let by_key = configured(
        "dim_customer",
        &[],
        &[
            ("refresh", json!("incremental")),
            ("key", json!("customer_id")),
        ],
    );
    assert_eq!(refresh(&by_key).unwrap(), Refresh::Key);
    let plan = planned(&[by_key], &request(&["dim_customer"]));
    let step = step_of(&plan, "dim_customer");
    assert_eq!(
        step.incremental.as_ref().unwrap().mode,
        IncrementalMode::Key
    );
    assert_eq!(step.periods, None);
    assert_eq!(
        plan.valid().value()["steps"][0]["incremental"],
        json!({"mode": "key"})
    );
    assert!(plan.valid().value()["steps"][0].get("periods").is_none());
}

#[test]
fn mg_req_0654_incremental_needs_partition_or_key() {
    let neither = configured("fact_sales", &[], &[("refresh", json!("incremental"))]);
    let error = refresh_errors(refresh(&neither).unwrap_err()).remove(0);
    assert_eq!(error.code(), "MG-0604");
    assert_eq!(
        error.to_string(),
        "model `fact_sales` has `refresh = \"incremental\"` but neither `partition_by` nor \
         `key`; set `partition_by = \"<column>:<grain>\"` to recompute periods, or `key` to \
         merge by key"
    );
    let errors = refresh_errors(plan(&[neither], &request(&["fact_sales"])).unwrap_err());
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), "MG-0604");

    let both = configured(
        "fact_sales",
        &[],
        &[
            ("refresh", json!("incremental")),
            ("partition_by", json!("sold_at:month")),
            ("key", json!(["order_id", "line"])),
        ],
    );
    assert_eq!(
        refresh(&both).unwrap(),
        Refresh::Period {
            column: "sold_at".to_owned(),
            grain: Grain::Month,
        }
    );
}
