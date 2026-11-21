use std::fs;
use std::path::Path;

use super::{check, differences, expected, model, relative};
use crate::json::Json;
use crate::workspace::{self, tests::scratch};

/// Copies the fixture schemas of `mergus-contracts` into a scratch directory and generates
/// their types next to them. Returns the schema and generated directories.
fn generated_scratch(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = workspace::root().unwrap();
    let dir = scratch(name);
    let fixture = root.join("crates/mergus-contracts/tests/fixtures/schemas/sample/v1");
    let schemas = dir.join("schemas");
    let version = schemas.join("sample").join("v1");
    fs::create_dir_all(&version).unwrap();
    for file in ["schema.json", "VERSION"] {
        fs::copy(fixture.join(file), version.join(file)).unwrap();
    }
    let out = dir.join("generated");
    fs::create_dir_all(&out).unwrap();
    for (path, text) in expected(&root, &schemas, &out).unwrap() {
        fs::write(path, text).unwrap();
    }
    (schemas, out)
}

fn problems(schemas: &Path, out: &Path) -> Vec<String> {
    let root = workspace::root().unwrap();
    differences(&root, schemas, out)
        .unwrap()
        .into_iter()
        .map(|problem| problem.rsplit('/').next().unwrap().to_owned())
        .collect()
}

#[test]
fn mg_req_0202_generated_types_are_up_to_date() {
    check(&workspace::root().unwrap()).unwrap();
}

#[test]
fn mg_req_0202_fresh_generation_has_no_difference() {
    let (schemas, out) = generated_scratch("contracts-fresh");
    assert_eq!(problems(&schemas, &out), Vec::<String>::new());
}

#[test]
fn mg_req_0202_edited_missing_and_stale_files_are_reported() {
    let (schemas, out) = generated_scratch("contracts-stale");
    fs::write(out.join("sample_v1.rs"), "// edited\n").unwrap();
    fs::remove_file(out.join("mod.rs")).unwrap();
    fs::write(out.join("old_v1.rs"), "").unwrap();
    let hint = "run `cargo xtask contracts-generate`";
    assert_eq!(
        problems(&schemas, &out),
        [
            format!("mod.rs: missing; {hint}"),
            format!("sample_v1.rs: out of date; {hint}"),
            format!("old_v1.rs: generated from no schema; {hint}"),
        ]
    );
}

#[test]
fn mg_req_0202_an_empty_schema_directory_generates_an_empty_registry() {
    let root = workspace::root().unwrap();
    let dir = scratch("contracts-empty");
    let files = expected(&root, &dir, &dir.join("generated")).unwrap();
    assert_eq!(files.len(), 1);
    assert!(
        files[0]
            .1
            .contains("pub const CONTRACTS: &[crate::ContractSchema] = &[];")
    );
}

#[test]
fn mg_req_0204_the_generated_type_exports_the_full_version() {
    let (_, out) = generated_scratch("contracts-semver");
    let text = fs::read_to_string(out.join("sample_v1.rs")).unwrap();
    assert!(
        text.contains("const SEMANTIC_VERSION: &'static str = \"1.0.0\";"),
        "{text}"
    );
}

#[test]
fn mg_req_0204_a_missing_or_malformed_version_file_fails() {
    let (schemas, out) = generated_scratch("contracts-version");
    let root = workspace::root().unwrap();
    let file = schemas.join("sample").join("v1").join("VERSION");
    let failure = |text: Option<&str>| {
        match text {
            Some(text) => fs::write(&file, text).unwrap(),
            None => fs::remove_file(&file).unwrap(),
        }
        let Err(failure) = expected(&root, &schemas, &out) else {
            panic!("{text:?} is accepted");
        };
        let message = format!("{failure:?}");
        message.rsplit("/v1").next().unwrap().to_owned()
    };
    assert_eq!(
        failure(Some("1.0\n")),
        "/VERSION: `1.0` is not one line `MAJOR.MINOR.PATCH`"
    );
    assert_eq!(
        failure(Some("2.0.0\n")),
        "/VERSION: `2.0.0` does not have the major version 1 of its directory `v1`"
    );
    assert_eq!(failure(None), ": `VERSION` is missing");
}

fn model_error(schema: &str) -> String {
    model::items("sample", &Json::parse(schema).unwrap()).unwrap_err()
}

#[test]
fn mg_req_0202_schemas_outside_the_supported_subset_are_refused() {
    let open = r#"{"description": "d", "type": "object", "properties": {}}"#;
    assert_eq!(
        model_error(open),
        ": objects must set `additionalProperties: false`"
    );
    let pattern = r#"{"description": "d", "type": "object", "additionalProperties": false,
        "properties": {"a": {"description": "d", "type": "string", "pattern": "x"}}}"#;
    assert_eq!(
        model_error(pattern),
        "/properties/a: unsupported keyword `pattern`"
    );
    let undocumented = r#"{"description": "d", "type": "object", "additionalProperties": false,
        "properties": {"a": {"type": "string"}}}"#;
    assert_eq!(
        model_error(undocumented),
        "/properties/a: `description` is missing"
    );
    let required = r#"{"description": "d", "type": "object", "additionalProperties": false,
        "required": ["b"], "properties": {}}"#;
    assert_eq!(model_error(required), ": `required` names no property `b`");
}

#[test]
fn mg_req_0202_pascal_case_names() {
    assert_eq!(
        model::pascal("catalog_schema").as_deref(),
        Some("CatalogSchema")
    );
    assert_eq!(model::pascal("view-2").as_deref(), Some("View2"));
    assert_eq!(model::pascal("Upper"), None);
    assert_eq!(model::pascal("2nd"), None);
}

#[test]
fn mg_req_0189_include_paths_are_relative_with_slashes() {
    let root = Path::new("w");
    let out = root.join("crates").join("c").join("src").join("generated");
    let schema = root
        .join("schemas")
        .join("a")
        .join("v1")
        .join("schema.json");
    assert_eq!(
        relative(&out, &schema),
        "../../../../schemas/a/v1/schema.json"
    );
}
