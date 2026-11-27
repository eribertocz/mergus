//! Segments and boundaries of a model: MG-REQ-0631, MG-REQ-0632, MG-REQ-0635 and
//! MG-REQ-0747.

mod common;

use common::{planned, request, step, with_steps};
use std::collections::BTreeMap;

use mergus_plan::{Engine, ModelStep, PlanModel, segment};
use mergus_project::{Identifier, ModelOptions, Publication};
use serde_json::{Value, json};

/// `load` and `clean` on DuckDB, `score` on Polars, `publish` on DuckDB.
fn mixed() -> Vec<ModelStep> {
    let all = ["id INTEGER", "amount DOUBLE", "note VARCHAR", "day DATE"];
    vec![
        step("load", Engine::Duckdb, &all, &[]),
        step(
            "clean",
            Engine::Duckdb,
            &all,
            &[
                ("load", "id"),
                ("load", "amount"),
                ("load", "note"),
                ("load", "day"),
            ],
        ),
        step(
            "score",
            Engine::Polars,
            &["id INTEGER", "score DOUBLE"],
            &[("clean", "day"), ("clean", "id"), ("clean", "amount")],
        ),
        step(
            "publish",
            Engine::Duckdb,
            &["id INTEGER", "day DATE", "score DOUBLE"],
            &[("score", "score"), ("score", "id"), ("clean", "day")],
        ),
    ]
}

/// The plan JSON of the only step of `stg_orders` made of `steps`, whose boundaries carry
/// about `rows` rows.
fn planned_step(steps: Vec<ModelStep>, rows: Option<u64>) -> Value {
    let mut request = request(&["stg_orders"]);
    if let Some(rows) = rows {
        request.rows.insert("stg_orders".to_owned(), rows);
    }
    let plan = planned(&[with_steps("stg_orders", steps)], &request);
    plan.valid().value()["steps"][0].clone()
}

/// The names of `names`, owned.
fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn mg_req_0631_consecutive_steps_fused() {
    let found: Vec<(usize, Engine, Vec<String>)> = segment(&mixed())
        .segments
        .into_iter()
        .map(|segment| (segment.index, segment.engine, segment.steps))
        .collect();
    assert_eq!(
        found,
        vec![
            (0, Engine::Duckdb, owned(&["load", "clean"])),
            (1, Engine::Polars, owned(&["score"])),
            (2, Engine::Duckdb, owned(&["publish"])),
        ]
    );
    let step = planned_step(mixed(), None);
    assert_eq!(
        step["segments"][0],
        json!({"index": 0, "engine": "duckdb", "steps": ["load", "clean"]})
    );
    assert_eq!(step["segments"][1]["steps"], json!(["score"]));
    assert_eq!(step["segments"].as_array().map(Vec::len), Some(3));
}

#[test]
fn mg_req_0631_one_engine_is_one_segment() {
    let steps = vec![
        step("a", Engine::Polars, &["x BIGINT"], &[]),
        step("b", Engine::Polars, &["x BIGINT"], &[("a", "x")]),
    ];
    let planned = segment(&steps);
    assert_eq!(planned.segments.len(), 1);
    assert!(planned.boundaries.is_empty());
}

#[test]
fn mg_req_0632_boundary_transfers_used_columns_only() {
    let crossings: Vec<(usize, usize, Vec<String>)> = segment(&mixed())
        .boundaries
        .into_iter()
        .map(|crossing| {
            let names = crossing.columns.into_iter().map(|c| c.name).collect();
            (crossing.from, crossing.to, names)
        })
        .collect();
    assert_eq!(
        crossings,
        vec![
            (0, 1, owned(&["id", "amount", "day"])),
            (0, 2, owned(&["day"])),
            (1, 2, owned(&["id", "score"])),
        ]
    );
    let step = planned_step(mixed(), None);
    assert_eq!(
        step["boundaries"][0],
        json!({"from": 0, "to": 1, "columns": ["id", "amount", "day"], "transport": "parquet"})
    );
}

#[test]
fn mg_req_0747_transport_by_estimated_size() {
    let transports = |rows| {
        let step = planned_step(mixed(), rows);
        step["boundaries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|boundary| boundary["transport"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    // The first boundary carries an integer, a double and a date, so 16 bytes a row; the
    // second a date, 4 bytes; the third an integer and a double, 12 bytes.
    assert_eq!(transports(None), owned(&["parquet", "parquet", "parquet"]));
    assert_eq!(
        transports(Some((1 << 30) / 16)),
        owned(&["arrow", "arrow", "arrow"])
    );
    assert_eq!(
        transports(Some((1 << 30) / 16 + 1)),
        owned(&["parquet", "arrow", "arrow"])
    );
    let text = vec![
        step("a", Engine::Duckdb, &["s VARCHAR"], &[]),
        step("b", Engine::Polars, &["s VARCHAR"], &[("a", "s")]),
    ];
    let step = planned_step(text, Some((1 << 30) / 64 + 1));
    assert_eq!(step["boundaries"][0]["transport"], json!("parquet"));
}

#[test]
fn mg_req_0635_polars_level_fallback() {
    let step = planned_step(mixed(), None);
    assert_eq!(step["segments"][1]["polars_level"], json!("subprocess"));
    assert_eq!(step["segments"][1]["polars_reason"], json!("not_available"));
}

#[test]
fn mg_req_0631_sql_model_is_one_segment() {
    let options = ModelOptions {
        name: "stg_orders".to_owned(),
        path: "models/silver/stg_orders.sql".to_owned(),
        publication: Publication::Intermediate,
        options: BTreeMap::new(),
    };
    let identifier = Identifier::parse("mdl_aaaaaaaaaaaaaaaa").unwrap();
    let sql = PlanModel::from_project(&options, &identifier, "SELECT 1\n", &[]);
    let plan = planned(&[sql], &request(&["stg_orders"]));
    let step = &plan.valid().value()["steps"][0];
    assert_eq!(
        step["segments"],
        json!([{"index": 0, "engine": "duckdb", "steps": ["stg_orders"]}])
    );
    assert_eq!(step["boundaries"], json!([]));
}

#[test]
fn mg_req_0747_transport_counts_same_names_from_two_steps() {
    let steps = vec![
        step("a", Engine::Duckdb, &["x BIGINT"], &[]),
        step("b", Engine::Duckdb, &["x BIGINT"], &[]),
        step(
            "c",
            Engine::Polars,
            &["y DOUBLE"],
            &[("a", "x"), ("b", "x")],
        ),
    ];
    // Two columns of 8 bytes cross: 16 bytes a row.
    let at_limit = planned_step(steps.clone(), Some((1 << 30) / 16));
    assert_eq!(at_limit["boundaries"][0]["transport"], json!("arrow"));
    let above = planned_step(steps, Some((1 << 30) / 16 + 1));
    assert_eq!(above["boundaries"][0]["transport"], json!("parquet"));
}

#[test]
fn mg_req_0747_polars_fields_only_on_polars_segments() {
    let step = planned_step(mixed(), None);
    for segment in step["segments"].as_array().into_iter().flatten() {
        let polars = segment["engine"] == json!("polars");
        assert_eq!(segment.get("polars_level").is_some(), polars);
        assert_eq!(segment.get("polars_reason").is_some(), polars);
    }
}
