//! Reads a schema directory laid out as `schemas/README.md` describes, and lists every way
//! its contracts break MG-REQ-0201.

use std::path::{Path, PathBuf};

use mergus_contracts::Schema;
use mergus_ports::FileSystem;
use mergus_ports::real::RealFileSystem;
use serde_json::Value;

/// The `$schema` every contract schema declares.
const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";

/// The root of the workspace.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

/// The directory of the test fixtures of this crate.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Lists the problems of every contract version under `schemas`, each prefixed with
/// `<contract>/v<version>: `.
pub fn problems(schemas: &Path) -> Vec<String> {
    let fs = RealFileSystem;
    let mut problems = Vec::new();
    for contract in directories(&fs, schemas) {
        for version in directories(&fs, &contract) {
            let label = format!("{}/{}", name(&contract), name(&version));
            for problem in version_problems(&fs, &version) {
                problems.push(format!("{label}: {problem}"));
            }
        }
    }
    problems
}

fn version_problems(fs: &RealFileSystem, dir: &Path) -> Vec<String> {
    let text = read(fs, &dir.join("schema.json"));
    let document: Value = serde_json::from_str(&text).unwrap();
    let mut problems = Vec::new();
    if document.get("$schema").and_then(Value::as_str) != Some(DRAFT) {
        problems.push(format!("`$schema` is not `{DRAFT}`"));
    }
    open_objects(&document, "", &mut problems);
    for field in ["contract", "contract_version"] {
        let declared = document["properties"].get(field).is_some();
        let required = document["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|name| name == field));
        if !declared || !required {
            problems.push(format!("`{field}` is not a required property"));
        }
    }
    let schema = Schema::parse(&text).unwrap();
    for (kind, should_pass) in [("valid", true), ("invalid", false)] {
        let examples = files(fs, &dir.join(kind));
        if examples.is_empty() {
            problems.push(format!("no {kind} example"));
        }
        for example in examples {
            let value: Value = serde_json::from_str(&read(fs, &example)).unwrap();
            if schema.validate(&value).is_empty() != should_pass {
                problems.push(format!("{kind} example `{}` is not {kind}", name(&example)));
            }
        }
    }
    problems
}

/// Reports every object schema under `node` that does not set `additionalProperties: false`.
fn open_objects(node: &Value, pointer: &str, problems: &mut Vec<String>) {
    let Some(members) = node.as_object() else {
        return;
    };
    let is_object = members.contains_key("properties")
        || members.get("type").and_then(Value::as_str) == Some("object");
    if is_object && members.get("additionalProperties") != Some(&Value::Bool(false)) {
        problems.push(format!("the object at `{pointer}` is not closed"));
    }
    for keyword in ["properties", "$defs"] {
        if let Some(children) = members.get(keyword).and_then(Value::as_object) {
            for (key, child) in children {
                open_objects(child, &format!("{pointer}/{keyword}/{key}"), problems);
            }
        }
    }
    if let Some(items) = members.get("items") {
        open_objects(items, &format!("{pointer}/items"), problems);
    }
}

fn entries(fs: &RealFileSystem, dir: &Path) -> Vec<PathBuf> {
    if fs.exists(dir).unwrap() {
        fs.list_dir(dir).unwrap()
    } else {
        Vec::new()
    }
}

fn directories(fs: &RealFileSystem, dir: &Path) -> Vec<PathBuf> {
    entries(fs, dir)
        .into_iter()
        .filter(|path| path.extension().is_none())
        .collect()
}

fn files(fs: &RealFileSystem, dir: &Path) -> Vec<PathBuf> {
    entries(fs, dir)
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect()
}

fn read(fs: &RealFileSystem, path: &Path) -> String {
    String::from_utf8(fs.read(path).unwrap()).unwrap()
}

fn name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}
