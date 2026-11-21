//! Tests of the `contract-compatibility` guard (MG-REQ-0204, MG-REQ-0205) against the
//! fixtures under `xtask/fixtures/guards/contract-compatibility/`.

use super::CONTRACT_COMPATIBILITY;
use super::tests::{lines, run};
use crate::contracts::incompatibilities;
use crate::json::Json;

const PREFIX: &str = "MG-REQ-0205: `schemas/sample/v1/schema.json` changes incompatibly \
                      without a new major version:";

#[test]
fn mg_req_0205_minor_changes_and_new_major_versions_pass() {
    assert_eq!(
        run(CONTRACT_COMPATIBILITY, "minor-passes"),
        Vec::<String>::new()
    );
}

#[test]
fn mg_req_0205_incompatible_change_without_major_bump_fails() {
    let expected: Vec<String> = [
        "`/properties/name` is no longer required",
        "`/properties/kind` changes `enum`",
        "`/properties/ratio` is removed",
        "`/properties/public` changes `type`",
        "`/properties/owner` is removed",
        "`/properties/size` is added as a required property",
        "`/$defs/owner` is removed",
    ]
    .iter()
    .map(|problem| format!("{PREFIX} {problem}"))
    .collect();
    let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
    assert_eq!(
        run(CONTRACT_COMPATIBILITY, "major-not-bumped"),
        lines("contract-compatibility", &expected)
    );
}

#[test]
fn mg_req_0205_a_schema_that_is_not_json_fails() {
    let found = run(CONTRACT_COMPATIBILITY, "not-json");
    assert_eq!(found.len(), 1);
    assert!(
        found[0].starts_with(
            "contract-compatibility: MG-REQ-0205: `schemas/sample/v1/schema.json` is not JSON:"
        ),
        "{found:?}"
    );
}

fn breaks(old: &str, new: &str) -> Vec<String> {
    incompatibilities(&Json::parse(old).unwrap(), &Json::parse(new).unwrap())
}

#[test]
fn mg_req_0204_adding_an_optional_field_is_minor() {
    let old = r#"{"description": "a", "type": "object", "properties": {}}"#;
    let new = r#"{"description": "b", "title": "t", "type": "object",
                  "properties": {"x": {"description": "x", "type": "string"}},
                  "$defs": {"d": {"description": "d", "type": "string"}}}"#;
    assert_eq!(breaks(old, new), Vec::<String>::new());
    let reordered = r#"{"enum": ["b", "a"], "description": "c"}"#;
    assert_eq!(
        breaks(r#"{"enum": ["a", "b"]}"#, reordered),
        Vec::<String>::new()
    );
}

#[test]
fn mg_req_0204_removing_renaming_retyping_or_tightening_is_major() {
    let old = r#"{"type": "object", "required": ["a"], "properties": {
        "a": {"type": "string"},
        "b": {"type": "array", "items": {"type": "string"}},
        "c": {"enum": ["x", "y"]},
        "d": {"const": 1}}}"#;
    let new = r#"{"type": "object", "required": ["a", "b"], "properties": {
        "renamed": {"type": "string"},
        "b": {"type": "array", "items": {"type": "integer"}},
        "c": {"enum": ["x"]},
        "d": {"const": 2},
        "a2": {"type": "string"}}}"#;
    assert_eq!(
        breaks(old, new),
        [
            "`/properties/a` is removed",
            "`/properties/b` becomes required",
            "`/properties/b/items` changes `type`",
            "`/properties/c` changes `enum`",
            "`/properties/d` changes `const`",
        ]
    );
    assert_eq!(
        breaks(r#"{"type": "object"}"#, r#"{"type": "string"}"#),
        ["the root changes `type`"]
    );
}
