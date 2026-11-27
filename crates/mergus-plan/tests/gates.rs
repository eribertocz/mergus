//! The gates of stage 4: MG-REQ-0720, MG-REQ-0721 and MG-REQ-0722.

#![cfg(test)]

use std::collections::BTreeMap;

use mergus_plan::{
    Existence, Gate, GateCheck, GateError, GateInput, GateReport, GateResult, PlanError,
    RequiredSecret, evaluate,
};
use mergus_ports::Keychain;
use mergus_sim::SimKeychain;

/// The value of the secret `secret:crm` in the keychain, which no output of stage 4 may show.
const VALUE: &str = "crm-value-1234";

/// The existence map of `references` as the runtime builds it: a reference resolves when
/// the keychain holds an entry for its name. The value read here never enters the map.
fn existence_from(keychain: &SimKeychain, references: &[&str]) -> BTreeMap<String, Existence> {
    references
        .iter()
        .map(|reference| {
            let name = reference.strip_prefix("secret:").unwrap();
            let existence = match keychain.get("mergus", name).unwrap() {
                Some(_) => Existence::Resolves,
                None => Existence::Missing,
            };
            ((*reference).to_owned(), existence)
        })
        .collect()
}

fn required(reference: &str, model: &str) -> RequiredSecret {
    RequiredSecret {
        reference: reference.to_owned(),
        model: model.to_owned(),
        step: Some(format!("compute {model}")),
    }
}

/// An error of the gate `gate` found by the caller for `model`.
fn found(gate: Gate, code: &'static str, model: &str) -> GateError {
    GateError {
        gate,
        code,
        model: Some(model.to_owned()),
        step: Some(format!("compute {model}")),
        message: format!("{model} breaks the gate"),
        fix: "change the model".to_owned(),
    }
}

/// An input where every gate but credentials passes, `secret:crm` resolves and
/// `secret:lost` is missing.
fn input(required: Vec<RequiredSecret>) -> GateInput {
    let checks = Gate::ALL
        .iter()
        .filter(|gate| **gate != Gate::Credentials)
        .map(|gate| (*gate, GateCheck::Checked(Vec::new())))
        .collect();
    GateInput {
        checks,
        required,
        existence: BTreeMap::from([
            ("secret:crm".to_owned(), Existence::Resolves),
            ("secret:lost".to_owned(), Existence::Missing),
        ]),
    }
}

fn result(report: &GateReport, gate: Gate) -> &GateResult {
    &report.results().iter().find(|(g, _)| *g == gate).unwrap().1
}

#[test]
fn mg_req_0720_blocked_gate_prevents_execution() {
    let clean = input(vec![required("secret:crm", "stg_orders")]);
    let report = evaluate(&clean).unwrap();
    // Every gate of the table is evaluated, in the order of the table.
    let gates: Vec<Gate> = report.results().iter().map(|(gate, _)| *gate).collect();
    assert_eq!(gates, Gate::ALL.to_vec());
    assert!(!report.blocks());
    assert!(report.clear().is_ok());

    let mut blocked = clean.clone();
    blocked.checks.insert(
        Gate::SqlSafety,
        GateCheck::Checked(vec![found(Gate::SqlSafety, "MG-0308", "stg_orders")]),
    );
    let report = evaluate(&blocked).unwrap();
    assert!(report.blocks());
    // Stage 5 needs the clearance, which a blocking gate refuses.
    let refused = report.clear().unwrap_err();
    assert_eq!(refused.errors().len(), 1);
    assert_eq!(refused.errors()[0].gate, Gate::SqlSafety);
}

#[test]
fn mg_req_0720_later_phase_gate_does_not_block() {
    let clean = input(vec![required("secret:crm", "stg_orders")]);
    // A gate whose checks belong to a later phase reports it and does not block.
    let mut later = clean.clone();
    later.checks.insert(
        Gate::Pii,
        GateCheck::NotApplicable {
            phase: "0.9".to_owned(),
        },
    );
    let report = evaluate(&later).unwrap();
    let pii = result(&report, Gate::Pii);
    assert_eq!(
        pii,
        &GateResult::NotApplicable {
            phase: "0.9".to_owned()
        }
    );
    assert!(!pii.blocks());
    assert!(report.clear().is_ok());
    assert!(
        report
            .to_string()
            .contains("pii: not_applicable until phase 0.9\n"),
        "{report}"
    );

    // A gate left out is an internal error, never a silent pass.
    let mut missing = clean;
    missing.checks.remove(&Gate::Determinism);
    assert!(matches!(
        evaluate(&missing),
        Err(PlanError::GateNotGiven(Gate::Determinism))
    ));
}

#[test]
fn mg_req_0721_secret_existence_checked_only() {
    let keychain = SimKeychain::with_entries([("mergus", "crm", VALUE)]);
    let mut given = input(vec![
        required("secret:crm", "stg_orders"),
        required("secret:lost", "dim_customer"),
    ]);
    given.existence = existence_from(&keychain, &["secret:crm", "secret:lost"]);
    let report = evaluate(&given).unwrap();
    let GateResult::Blocked(errors) = result(&report, Gate::Credentials) else {
        panic!("the credentials gate does not block: {report}");
    };
    assert_eq!(errors.len(), 1);
    let error = &errors[0];
    assert_eq!(error.code, "MG-0701");
    assert_eq!(error.gate, Gate::Credentials);
    assert_eq!(error.model.as_deref(), Some("dim_customer"));
    assert_eq!(error.step.as_deref(), Some("compute dim_customer"));
    assert_eq!(
        error.to_string(),
        "MG-0701: gate `credentials`, model `dim_customer`, step `compute dim_customer`: \
         the secret `secret:lost` does not resolve; fix: run `mergus secret set lost`"
    );
    // No value appears in any output of the gates: the input, the report, its errors.
    let outputs = [
        format!("{given:?}"),
        report.to_string(),
        format!("{report:?}"),
        report.clear().unwrap_err().to_string(),
        format!("{:?}", report.clear().unwrap_err()),
    ];
    for output in outputs {
        assert!(!output.contains(VALUE), "{output}");
    }
    // A required reference the existence map does not answer is an internal error.
    let unknown = input(vec![required("secret:other", "stg_orders")]);
    assert!(matches!(
        evaluate(&unknown),
        Err(PlanError::ExistenceNotGiven(reference)) if reference == "secret:other"
    ));
}

#[test]
fn mg_req_0722_all_gate_errors_reported() {
    let mut given = input(vec![
        required("secret:lost", "stg_orders"),
        required("secret:lost", "fact_sales"),
    ]);
    given.checks.insert(
        Gate::SqlSafety,
        GateCheck::Checked(vec![
            found(Gate::SqlSafety, "MG-0308", "stg_orders"),
            found(Gate::SqlSafety, "MG-0308", "fact_sales"),
        ]),
    );
    let refused = evaluate(&given).unwrap().clear().unwrap_err();
    let codes: Vec<(Gate, &str, Option<&str>)> = refused
        .errors()
        .iter()
        .map(|error| (error.gate, error.code, error.model.as_deref()))
        .collect();
    assert_eq!(
        codes,
        [
            (Gate::SqlSafety, "MG-0308", Some("stg_orders")),
            (Gate::SqlSafety, "MG-0308", Some("fact_sales")),
            (Gate::Credentials, "MG-0701", Some("fact_sales")),
            (Gate::Credentials, "MG-0701", Some("stg_orders")),
        ]
    );
    // One output holds every error, one per line.
    let shown = refused.to_string();
    assert_eq!(shown.lines().count(), 5, "{shown}");
    assert!(
        shown.starts_with("stage 4 blocked: 4 errors in 2 gates\n"),
        "{shown}"
    );
}

#[test]
fn mg_req_0720_errors_filed_under_another_gate_are_rejected() {
    let mut given = input(Vec::new());
    given.checks.insert(
        Gate::Pii,
        GateCheck::Checked(vec![found(Gate::SqlSafety, "MG-0308", "stg_orders")]),
    );
    assert!(matches!(
        evaluate(&given),
        Err(PlanError::GateMismatch {
            filed: Gate::Pii,
            found: Gate::SqlSafety
        })
    ));
}

#[test]
fn mg_req_0720_not_applicable_needs_a_later_phase() {
    let with_phase = |phase: &str| {
        let mut given = input(Vec::new());
        given.checks.insert(
            Gate::Determinism,
            GateCheck::NotApplicable {
                phase: phase.to_owned(),
            },
        );
        evaluate(&given)
    };
    // Gates due in 0.4 or before are always evaluated.
    for phase in ["", "0.4", "0.4.0", "0.3", "0.1.9", "x", "0.", "1..2"] {
        assert!(
            matches!(
                with_phase(phase),
                Err(PlanError::PhaseNotLater {
                    gate: Gate::Determinism,
                    ..
                })
            ),
            "{phase:?}"
        );
    }
    // Phases are compared as versions, not as text.
    for phase in ["0.5", "0.10", "0.4.1", "1.0"] {
        assert!(with_phase(phase).is_ok(), "{phase:?}");
    }
}
