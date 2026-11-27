//! Refresh: MG-REQ-0651 to MG-REQ-0657 and MG-REQ-0661.

mod common;

use common::{model, names, planned, request};
use mergus_contracts::generated::mergus_plan_v1::{IncrementalMode, Step, StepAction, StepRefresh};
use mergus_plan::{
    Backfill, By, Deletes, Grain, Instant, PlanError, PlanModel, Range, Refresh, RefreshError,
    Storage, plan, refresh,
};
use serde_json::{Value, json};

/// `model` with the options `options` set.
fn configured(name: &str, inputs: &[&str], options: &[(&str, Value)]) -> PlanModel {
    let mut made = model(name, inputs);
    for (option, value) in options {
        made.config.insert((*option).to_owned(), value.clone());
    }
    made
}

/// An incremental model `fact_sales` by day of `sold_at`, reading `stg_orders` and
/// `dim_customer`.
fn daily() -> PlanModel {
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
fn day(year: i32, month: u32, day: u32) -> Instant {
    Instant::from_date(year, month, day).unwrap()
}

/// The range `[start, end)`.
fn range(start: Instant, end: Instant) -> Range {
    Range::new(start, end).unwrap()
}

/// The step of `name` in `plan`.
fn step_of<'a>(plan: &'a mergus_plan::Plan, name: &str) -> &'a Step {
    plan.document()
        .steps
        .iter()
        .find(|step| step.model == name)
        .unwrap()
}

/// The `(start, end)` of each period of `step`.
fn periods(step: &Step) -> Vec<(String, String)> {
    step.periods
        .as_ref()
        .unwrap()
        .iter()
        .map(|period| (period.start.clone(), period.end.clone()))
        .collect()
}

/// The pair of RFC 3339 texts.
fn texts(start: &str, end: &str) -> (String, String) {
    (start.to_owned(), end.to_owned())
}

/// The refresh errors of a failed plan.
fn refresh_errors(error: PlanError) -> Vec<RefreshError> {
    match error {
        PlanError::Refresh(errors) => errors,
        other => panic!("not a refresh error: {other}"),
    }
}

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
    assert_eq!(error.code(), "MG-0704");
    assert_eq!(
        error.to_string(),
        "model `fact_sales` has `refresh = \"incremental\"` but neither `partition_by` nor \
         `key`; set `partition_by = \"<column>:<grain>\"` to recompute periods, or `key` to \
         merge by key"
    );
    let errors = refresh_errors(plan(&[neither], &request(&["fact_sales"])).unwrap_err());
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code(), "MG-0704");

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
        "model `fact_sales` cannot be backfilled: it is not incremental by period; give it \
         `refresh = \"incremental\"` and `partition_by = \"<column>:<grain>\"`"
    );
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

/// The error of planning every model of `models`.
fn plan_of(models: &[PlanModel]) -> PlanError {
    let all: Vec<&str> = models.iter().map(|model| model.name.as_str()).collect();
    plan(models, &request(&all)).unwrap_err()
}
