//! The validator follows JSON Schema 2020-12 for the supported keywords, and refuses every
//! other keyword (MG-REQ-0203).

use mergus_contracts::{Schema, SchemaError};
use serde_json::json;

fn violations(schema: &str, document: serde_json::Value) -> Result<Vec<String>, SchemaError> {
    Ok(Schema::parse(schema)?
        .validate(&document)
        .iter()
        .map(ToString::to_string)
        .collect())
}

const SCHEMA: &str = r##"{
    "type": "object",
    "additionalProperties": false,
    "required": ["count", "items"],
    "properties": {
        "count": {"type": "integer"},
        "flag": {"type": "boolean"},
        "items": {"type": "array", "items": {"$ref": "#/$defs/item"}},
        "a/b": {"const": null}
    },
    "$defs": {
        "item": {"type": "object", "additionalProperties": false,
                 "properties": {"kind": {"enum": ["x", "y"]}}}
    }
}"##;

#[test]
fn mg_req_0203_valid_documents_have_no_violation() {
    let document = json!({"count": 1.0, "items": [{"kind": "x"}, {}], "a/b": null});
    assert_eq!(violations(SCHEMA, document).unwrap(), Vec::<String>::new());
}

#[test]
fn mg_req_0203_every_violation_is_reported_with_its_pointer() {
    let document = json!({
        "count": 1.5,
        "flag": "yes",
        "items": [{"kind": "z"}, {"other": 1}],
        "a/b": 0,
        "extra": true
    });
    assert_eq!(
        violations(SCHEMA, document).unwrap(),
        [
            "/a~1b: the value is not the constant null",
            "/count: the value is not of type `integer`",
            "/extra: the property is not allowed",
            "/flag: the value is not of type `boolean`",
            "/items/0/kind: the value is not one of the values of `enum`",
            "/items/1/other: the property is not allowed",
        ]
    );
    assert_eq!(
        violations(SCHEMA, json!([])).unwrap(),
        [": the value is not of type `object`"]
    );
    assert_eq!(
        violations(SCHEMA, json!({"items": []})).unwrap(),
        [": the required property `count` is missing"]
    );
}

fn refusal(schema: &str) -> SchemaError {
    match Schema::parse(schema) {
        Ok(parsed) => SchemaError {
            pointer: String::new(),
            message: format!("accepted: {parsed:?}"),
        },
        Err(error) => error,
    }
}

#[test]
fn mg_req_0203_unsupported_schemas_are_refused() {
    let error = refusal(r#"{"properties": {"a": {"pattern": "x"}}}"#);
    assert_eq!(error.pointer, "/properties/a");
    assert_eq!(error.message, "unsupported keyword `pattern`");
    let error = refusal(r##"{"$ref": "#/$defs/missing"}"##);
    assert_eq!(error.message, "`$ref` names no definition `missing`");
    let error = refusal(r#"{"additionalProperties": true}"#);
    assert_eq!(error.message, "`additionalProperties` must be `false`");
    let error = refusal(r#"{"properties": {"a": {"$defs": {}}}}"#);
    assert_eq!(error.message, "unsupported keyword `$defs`");
    let error = refusal(r#"{"type": ["string", "null"]}"#);
    assert_eq!(error.pointer, "/type");
    assert!(refusal("not json").message.starts_with("not JSON"));
}
