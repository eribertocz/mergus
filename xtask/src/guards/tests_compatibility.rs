//! Tests of the `contract-compatibility` guard (MG-REQ-0204, MG-REQ-0205) against the
//! fixtures under `xtask/fixtures/guards/contract-compatibility/`.

use super::CONTRACT_COMPATIBILITY;
use super::tests::{lines, run};
use crate::contracts::{Bump, classify};
use crate::json::Json;

const PREFIX: &str = "MG-REQ-0205: `schemas/sample/v1/schema.json` changes incompatibly \
                      without a new major version:";

#[test]
fn mg_req_0205_matching_bumps_and_new_major_versions_pass() {
    for case in ["minor-passes", "patch-passes"] {
        assert_eq!(
            run(CONTRACT_COMPATIBILITY, case),
            Vec::<String>::new(),
            "{case}"
        );
    }
}

/// Runs the guard on a fixture and compares its findings with `expected`.
fn finds(case: &str, expected: &[&str]) {
    let expected: Vec<String> = expected
        .iter()
        .map(|finding| format!("MG-REQ-0205: {finding}"))
        .collect();
    let expected: Vec<&str> = expected.iter().map(String::as_str).collect();
    assert_eq!(
        run(CONTRACT_COMPATIBILITY, case),
        lines("contract-compatibility", &expected),
        "{case}"
    );
}

#[test]
fn mg_req_0205_a_change_without_a_version_bump_fails() {
    finds(
        "version-not-bumped",
        &[
            "`schemas/sample/v1/VERSION` is unchanged, but a minor change to `schema.json` \
           needs a new version",
        ],
    );
}

#[test]
fn mg_req_0205_a_bump_that_does_not_match_the_change_fails() {
    finds(
        "version-wrong-increment",
        &[
            "`schemas/sample/v1/VERSION` goes from `1.0.0` to `1.0.1`, but a minor change to \
             `schema.json` needs `1.1.0`",
            "`schemas/other/v1/VERSION` goes from `1.2.3` to `1.3.3`, but a minor change to \
             `schema.json` needs `1.3.0`",
        ],
    );
    finds(
        "version-without-change",
        &[
            "`schemas/sample/v1/VERSION` goes from `1.0.0` to `1.1.0`, but no change to \
           `schema.json` needs `1.0.0`",
        ],
    );
}

#[test]
fn mg_req_0205_a_new_contract_or_major_version_starts_at_major_0_0() {
    finds(
        "new-version-wrong-start",
        &[
            "`schemas/sample/v2/VERSION` is `2.1.0`, but a new contract or major version \
             starts at `2.0.0`",
            "`schemas/other/v1/VERSION` is `1.0.1`, but a new contract or major version \
             starts at `1.0.0`",
        ],
    );
}

#[test]
fn mg_req_0205_a_missing_version_file_fails() {
    finds(
        "version-missing",
        &[
            "`schemas/other/v1/VERSION` is missing",
            "`schemas/sample/v1/VERSION` is missing",
            "`schemas/third/v1/VERSION` is missing at the base",
        ],
    );
}

#[test]
fn mg_req_0205_a_malformed_version_file_fails() {
    finds(
        "version-malformed",
        &[
            "`schemas/sample/v1/VERSION` is malformed: `1.1` is not one line \
             `MAJOR.MINOR.PATCH`",
            "`schemas/other/v1/VERSION` is malformed: `2.0.0` does not have the major version \
             1 of its directory `v1`",
            "`schemas/third/v1/VERSION` at the base is malformed: `one` is not one line \
             `MAJOR.MINOR.PATCH`",
        ],
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

fn change(old: &str, new: &str) -> Result<Bump, Vec<String>> {
    classify(&Json::parse(old).unwrap(), &Json::parse(new).unwrap())
}

fn breaks(old: &str, new: &str) -> Vec<String> {
    change(old, new).err().unwrap_or_default()
}

#[test]
fn mg_req_0204_changes_are_classified_as_patch_minor_or_none() {
    let old = r#"{"description": "a", "type": "object", "properties": {
        "x": {"description": "x", "type": "string"}}}"#;
    assert_eq!(change(old, old), Ok(Bump::None));
    let described = r#"{"description": "b", "title": "t", "type": "object", "properties": {
        "x": {"description": "y", "type": "string"}}}"#;
    assert_eq!(change(old, described), Ok(Bump::Patch));
    let property = r#"{"description": "a", "type": "object", "properties": {
        "x": {"description": "x", "type": "string"},
        "y": {"description": "y", "type": "string"}}}"#;
    assert_eq!(change(old, property), Ok(Bump::Minor));
    let definition = r#"{"description": "a", "type": "object", "properties": {
        "x": {"description": "x", "type": "string"}},
        "$defs": {"d": {"description": "d", "type": "string"}}}"#;
    assert_eq!(change(old, definition), Ok(Bump::Minor));
    let loosened = r#"{"description": "a", "type": "object", "properties": {
        "x": {"description": "x", "type": "string", "enum": ["a"]}}}"#;
    assert!(change(old, loosened).is_err());
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
