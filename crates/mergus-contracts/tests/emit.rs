//! MG-REQ-0203: a contract document is validated against its schema before it can be
//! emitted, and one that fails validation is never emitted.

use mergus_contracts::{Contract, ContractSchema, EmitError, ValidDocument};

#[path = "fixtures/generated/mod.rs"]
mod generated;

use generated::sample_v1::{Owner, Sample, SampleKind, SampleLocation};

fn sample() -> Sample {
    Sample {
        contract: "sample".to_owned(),
        contract_version: 1,
        name: "orders".to_owned(),
        kind: SampleKind::View,
        location: SampleLocation {
            path: "sales/orders".to_owned(),
            depth: Some(2),
        },
        ratio: Some(0.5),
        public: Some(true),
        tags: Some(vec!["sales".to_owned(), "daily".to_owned()]),
        owner: Some(Owner {
            team: "finance".to_owned(),
        }),
    }
}

/// Emits `document` into `out` the only way the crate allows: through a [`ValidDocument`].
fn emit<T: Contract>(document: &T, out: &mut Vec<String>) -> Result<(), EmitError> {
    let valid: ValidDocument = mergus_contracts::validate(document)?;
    out.push(valid.value().to_string());
    Ok(())
}

#[test]
fn mg_req_0203_invalid_document_never_emitted() {
    let mut out = Vec::new();
    let mut document = sample();
    document.contract = "other".to_owned();
    let error = emit(&document, &mut out).unwrap_err();
    assert!(out.is_empty());
    let EmitError::Invalid {
        contract,
        violations,
    } = &error
    else {
        panic!("unexpected error: {error}");
    };
    assert_eq!(*contract, "sample");
    let violations: Vec<String> = violations.iter().map(ToString::to_string).collect();
    assert_eq!(
        violations,
        ["/contract: the value is not the constant \"sample\""]
    );
    assert!(error.to_string().starts_with("internal error:"));
}

#[test]
fn mg_req_0203_valid_document_is_emitted_as_serialized() {
    let mut out = Vec::new();
    emit(&sample(), &mut out).unwrap();
    let full: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schemas/sample/v1/valid/full.json")).unwrap();
    assert_eq!(out, [full.to_string()]);
}

#[test]
fn mg_req_0203_absent_optional_fields_are_not_emitted() {
    let mut document = sample();
    document.location.depth = None;
    document.ratio = None;
    document.public = None;
    document.tags = None;
    document.owner = None;
    let value = mergus_contracts::validate(&document).unwrap().into_value();
    let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        ["contract", "contract_version", "kind", "location", "name"]
    );
}

#[test]
fn mg_req_0202_registry_lists_the_generated_contracts() {
    let names: Vec<(&str, u32)> = generated::CONTRACTS
        .iter()
        .map(|schema: &ContractSchema| (schema.name, schema.version))
        .collect();
    assert_eq!(names, [("sample", 1)]);
    assert_eq!(generated::CONTRACTS[0].schema, Sample::SCHEMA);
}
