//! MG-REQ-0206: a reader accepts every document of an older minor version of the same major
//! version, and refuses a document of another major version.

use std::path::{Path, PathBuf};

use mergus_contracts::{Contract, ReadError, Schema, read};
use mergus_ports::FileSystem;
use mergus_ports::real::RealFileSystem;
use serde_json::{Value, json};

#[path = "fixtures/generated/sample_v1.rs"]
mod sample_v1;

use sample_v1::{Sample, SampleKind};

/// The fixtures of an older minor version of the `sample` contract.
fn older_minor() -> PathBuf {
    ["tests", "fixtures", "older_minor", "sample", "v1"]
        .iter()
        .fold(
            Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf(),
            |path, part| path.join(part),
        )
}

/// The documents an older minor version of the `sample` contract accepts.
#[cfg(test)]
fn older_minor_documents() -> Vec<Value> {
    let fs = RealFileSystem;
    let dir = older_minor().join("valid");
    let documents: Vec<Value> = fs
        .list_dir(&dir)
        .unwrap()
        .iter()
        .map(|path| serde_json::from_slice(&fs.read(path).unwrap()).unwrap())
        .collect();
    assert!(!documents.is_empty());
    documents
}

#[test]
fn mg_req_0206_older_minor_versions_accepted() {
    let fs = RealFileSystem;
    let older = fs.read(&older_minor().join("schema.json")).unwrap();
    let older = Schema::parse(std::str::from_utf8(&older).unwrap()).unwrap();
    for document in older_minor_documents() {
        assert!(older.validate(&document).is_empty());
        let sample: Sample = read(&document).unwrap();
        assert_eq!(sample.contract_version, 1);
        assert_eq!(sample.tags, None);
        assert_eq!(sample.owner, None);
    }
}

#[test]
fn mg_req_0206_the_latest_minor_version_is_read() {
    let document = json!({
        "contract": "sample",
        "contract_version": 1,
        "name": "orders",
        "kind": "view",
        "location": { "path": "sales/orders", "depth": 2 },
        "tags": ["daily"],
        "owner": { "team": "finance" }
    });
    let sample: Sample = read(&document).unwrap();
    assert_eq!(sample.kind, SampleKind::View);
    assert_eq!(sample.location.depth, Some(2));
}

#[test]
fn mg_req_0206_other_major_versions_and_unknown_fields_refused() {
    let mut document = older_minor_documents().remove(0);
    document["contract_version"] = json!(2);
    let error = read::<Sample>(&document).unwrap_err();
    assert!(matches!(
        error,
        ReadError::Invalid {
            contract: "sample",
            ..
        }
    ));
    assert!(error.to_string().contains("/contract_version"));
    document["contract_version"] = json!(1);
    document["added_later"] = json!(true);
    assert!(matches!(
        read::<Sample>(&document),
        Err(ReadError::Invalid { .. })
    ));
}
