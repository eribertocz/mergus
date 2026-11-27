//! The gates of stage 4: MG-REQ-0720, MG-REQ-0721 and MG-REQ-0722.

#![cfg(test)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use mergus_plan::{Gate, GateError, GateInput, GateResult, RequiredSecret, SecretCheck, evaluate};
use mergus_security::{SecretRef, origin};
use mergus_sim::{SimEnvironment, SimKeychain};

/// The check of `mergus doctor` (MG-REQ-1004) over simulated ports, recording each name asked.
struct DoctorCheck {
    environment: SimEnvironment,
    keychain: SimKeychain,
    asked: RefCell<Vec<String>>,
}

impl SecretCheck for DoctorCheck {
    fn resolves(&self, name: &str) -> bool {
        self.asked.borrow_mut().push(name.to_owned());
        SecretRef::from_name(name)
            .is_some_and(|reference| origin(&reference, &self.environment, &self.keychain).is_ok())
    }
}

/// A check where the secret `crm` is in the environment and `erp` in the keychain, both
/// with a value no output may show.
fn doctor() -> DoctorCheck {
    let variable = SecretRef::from_name("crm").unwrap().variable();
    DoctorCheck {
        environment: SimEnvironment::new([(variable, "env-value-1234")], None),
        keychain: SimKeychain::with_entries([("mergus", "erp", "keychain-value-5678")]),
        asked: RefCell::new(Vec::new()),
    }
}

fn required(name: &str, model: &str) -> RequiredSecret {
    RequiredSecret {
        name: name.to_owned(),
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

#[test]
fn mg_req_0720_blocked_gate_prevents_execution() {
    let check = doctor();
    let clean = GateInput {
        checked: BTreeMap::from([(Gate::SqlSafety, Vec::new())]),
        secrets: vec![required("crm", "stg_orders")],
    };
    let report = evaluate(&clean, &check);
    // Every gate of the table is evaluated, in the order of the table.
    let gates: Vec<Gate> = report.results().iter().map(|(gate, _)| *gate).collect();
    assert_eq!(gates, Gate::ALL.to_vec());
    assert!(!report.blocks());
    assert!(report.clear().is_ok());

    let blocked = GateInput {
        checked: BTreeMap::from([(
            Gate::SqlSafety,
            vec![found(Gate::SqlSafety, "MG-0308", "stg_orders")],
        )]),
        secrets: vec![required("crm", "stg_orders")],
    };
    let report = evaluate(&blocked, &check);
    assert!(report.blocks());
    // Stage 5 needs the clearance, which a blocking gate refuses.
    let refused = report.clear().unwrap_err();
    assert_eq!(refused.errors().len(), 1);
    assert_eq!(refused.errors()[0].gate, Gate::SqlSafety);
}

#[test]
fn mg_req_0720_gates_of_later_phases_pass_and_say_so() {
    let report = evaluate(&GateInput::default(), &doctor());
    for (gate, result) in report.results() {
        if *gate == Gate::Credentials {
            assert_eq!(result, &GateResult::Passed);
        } else {
            assert_eq!(result, &GateResult::Deferred, "{gate}");
            assert!(!result.blocks());
        }
    }
    let shown = report.to_string();
    assert!(
        shown.contains("pii: passes, its check belongs to a later phase"),
        "{shown}"
    );
    assert!(shown.contains("credentials: passes\n"), "{shown}");
}

#[test]
fn mg_req_0721_secret_existence_checked_only() {
    let check = doctor();
    let input = GateInput {
        checked: BTreeMap::new(),
        secrets: vec![
            required("erp", "fact_sales"),
            required("crm", "stg_orders"),
            required("missing", "dim_customer"),
        ],
    };
    let report = evaluate(&input, &check);
    // Each reference is asked once, through the existence check only.
    assert_eq!(*check.asked.borrow(), ["crm", "erp", "missing"]);
    let (_, credentials) = report
        .results()
        .iter()
        .find(|(gate, _)| *gate == Gate::Credentials)
        .unwrap();
    let GateResult::Blocked(errors) = credentials else {
        panic!("the credentials gate does not block: {credentials:?}");
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
         the secret `secret:missing` does not resolve; fix: run `mergus secret set missing`"
    );
    // No value is ever shown, in any output of the gates.
    let outputs = [
        report.to_string(),
        format!("{report:?}"),
        report.clear().unwrap_err().to_string(),
    ];
    for output in outputs {
        assert!(!output.contains("env-value-1234"), "{output}");
        assert!(!output.contains("keychain-value-5678"), "{output}");
    }
}

#[test]
fn mg_req_0722_all_gate_errors_reported() {
    let input = GateInput {
        checked: BTreeMap::from([
            (
                Gate::SqlSafety,
                vec![
                    found(Gate::SqlSafety, "MG-0308", "stg_orders"),
                    found(Gate::SqlSafety, "MG-0308", "fact_sales"),
                ],
            ),
            (Gate::TypePassport, Vec::new()),
        ]),
        secrets: vec![
            required("gone", "fact_sales"),
            required("lost", "stg_orders"),
        ],
    };
    let refused = evaluate(&input, &doctor()).clear().unwrap_err();
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
    assert!(shown.contains("secret `secret:gone`"), "{shown}");
    assert!(shown.contains("secret `secret:lost`"), "{shown}");
}
