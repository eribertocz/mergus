//! Fingerprints: MG-REQ-0711, MG-REQ-0712 and MG-REQ-0743.

mod common;

use std::collections::BTreeMap;

use common::{fingerprint_of, model, planned, request, shop};
use mergus_plan::{FingerprintInputs, PlanModel, fingerprint, sha256_hex};
use mergus_project::{Effective, Identifier, ModelOptions, Origin, Publication, Setting};
use serde_json::{Value, json};

/// The fingerprint of `fact_sales` in the plan of `models` with the raw version and engine
/// versions of `edit`.
fn fact_sales(models: &[PlanModel], edit: impl Fn(&mut mergus_plan::PlanRequest)) -> String {
    let mut request = request(&["dim_customer", "fact_sales", "stg_orders"]);
    edit(&mut request);
    fingerprint_of(&planned(models, &request), "fact_sales")
}

#[test]
fn mg_req_0711_fingerprint_changes_with_each_input() {
    let base = fact_sales(&shop(), |_| {});
    assert_eq!(base, fact_sales(&shop(), |_| {}));
    // Its code and its configuration.
    let mut code = shop();
    code[2].code.push_str("WHERE 1 = 1\n");
    let mut config = shop();
    config[2]
        .config
        .insert("tags".to_owned(), json!(["finance"]));
    // The fingerprint of an upstream model, here changed by the code of `stg_orders`.
    let mut upstream = shop();
    upstream[0].code.push_str("-- changed\n");
    let changed = [
        fact_sales(&code, |_| {}),
        fact_sales(&config, |_| {}),
        fact_sales(&upstream, |_| {}),
        // The version of a raw input, which reaches `fact_sales` through `stg_orders`.
        fact_sales(&shop(), |request| {
            request
                .versions
                .insert("raw.erp__orders".to_owned(), "v2".to_owned());
        }),
        // The engine versions.
        fact_sales(&shop(), |request| {
            request
                .engines
                .insert("duckdb".to_owned(), "1.2.0".to_owned());
        }),
    ];
    for (index, print) in changed.iter().enumerate() {
        assert_ne!(*print, base, "case {index}");
    }
}

#[test]
fn mg_req_0712_unchanged_model_skipped() {
    let models = shop();
    let first = planned(&models, &request(&["stg_orders"]));
    let print = fingerprint_of(&first, "stg_orders");
    let mut again = request(&["stg_orders"]);
    again.available.insert(print.clone());
    let second = planned(&models, &again);
    let step = &second.document().steps[0];
    assert_eq!(step.fingerprint, print);
    assert_eq!(serde_json::to_value(step.action).unwrap(), json!("skip"));
}

/// The fingerprint of the object built from these parts.
fn of(
    code: &str,
    config: &[(&str, Value)],
    inputs: &[(&str, Option<&str>)],
    parameters: &[(&str, Value)],
    engines: &[(&str, &str)],
    output_contract: Option<&str>,
) -> String {
    let owned = |pairs: &[(&str, Value)]| -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect()
    };
    let inputs = inputs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.map(str::to_owned)))
        .collect();
    let engines = engines
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    fingerprint(&FingerprintInputs {
        code,
        config: &owned(config),
        inputs: &inputs,
        parameters: &owned(parameters),
        engines: &engines,
        output_contract,
    })
}

#[test]
fn mg_req_0743_plan_id_and_fingerprint_inputs() {
    // The encoding: SHA-256 (FIPS 180-2 vector) in lowercase hexadecimal, of canonical JSON.
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let empty = of("SELECT 1", &[], &[], &[], &[], None);
    let expected = r#"{"code":"SELECT 1","config":{},"engines":{},"inputs":{},"output_contract":null,"parameters":{}}"#;
    assert_eq!(empty, sha256_hex(expected.as_bytes()));
    let mut bare = model("a", &[]);
    bare.code = "SELECT 1".to_owned();
    bare.config.clear();
    assert_eq!(
        fingerprint_of(&planned(&[bare], &request(&["a"])), "a"),
        empty
    );
    // `code`: CRLF and LF give the same fingerprint, any other change another.
    let mut crlf = model("a", &[]);
    crlf.code = "SELECT 1\r\nFROM t\r\n".to_owned();
    let lf = mergus_plan::lf_text(&crlf.code);
    assert_eq!(lf, "SELECT 1\nFROM t\n");
    assert_ne!(of("SELECT 2", &[], &[], &[], &[], None), empty);
    // `config`.
    assert_ne!(
        of("SELECT 1", &[("tags", json!(["a"]))], &[], &[], &[], None),
        empty
    );
    // `inputs`: an upstream fingerprint or a version, and `null`.
    let null = of("SELECT 1", &[], &[("raw.t", None)], &[], &[], None);
    let version = of("SELECT 1", &[], &[("raw.t", Some("v1"))], &[], &[], None);
    assert!(null != empty && version != empty && null != version);
    // `parameters`.
    assert_ne!(
        of(
            "SELECT 1",
            &[],
            &[],
            &[("day", json!("2026-01-01"))],
            &[],
            None
        ),
        empty
    );
    // `engines`.
    assert_ne!(
        of("SELECT 1", &[], &[], &[], &[("duckdb", "1.2.0")], None),
        empty
    );
    // `output_contract`.
    assert_ne!(of("SELECT 1", &[], &[], &[], &[], Some("1.0.0")), empty);
    // Nothing else: neither the origins of the options nor the identifier.
    let options = |origin: Origin| ModelOptions {
        name: "a".to_owned(),
        path: "models/a.sql".to_owned(),
        publication: Publication::Intermediate,
        options: BTreeMap::from([
            (
                "refresh".to_owned(),
                Effective::Single(Setting {
                    value: json!("full"),
                    origin: origin.clone(),
                }),
            ),
            (
                "tags".to_owned(),
                Effective::List(vec![
                    Setting {
                        value: json!("z"),
                        origin: origin.clone(),
                    },
                    Setting {
                        value: json!("a"),
                        origin,
                    },
                ]),
            ),
        ]),
    };
    let first = Identifier::parse("mdl_aaaaaaaaaaaaaaaa").unwrap();
    let second = Identifier::parse("mdl_bbbbbbbbbbbbbbbb").unwrap();
    let file = Origin::File {
        path: "models/mergus.toml".to_owned(),
        line: 3,
    };
    let one = PlanModel::from_project(&options(file), &first, "SELECT 1\r\n", &[]);
    let two = PlanModel::from_project(&options(Origin::Default), &second, "SELECT 1\n", &[]);
    assert_eq!(one.code, two.code);
    assert_eq!(one.config, two.config);
    assert_eq!(one.config["tags"], json!(["z", "a"]), "union order is kept");
    let print = |model: PlanModel| fingerprint_of(&planned(&[model], &request(&["a"])), "a");
    assert_eq!(print(one), print(two));
    // `plan_id`: every other field enters it, `plan_id` itself does not.
    let models = shop();
    let dev = planned(&models, &request(&["fact_sales"]));
    let mut other = request(&["fact_sales"]);
    other.environment = "prod".to_owned();
    assert_ne!(dev.plan_id(), planned(&models, &other).plan_id());
    let mut value = dev.valid().value().clone();
    value.as_object_mut().unwrap().remove("plan_id");
    let canonical = mergus_contracts::canonical_json(&value);
    assert_eq!(dev.plan_id(), sha256_hex(canonical.as_bytes()));
}
