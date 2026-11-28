//! MG-REQ-1313: idempotency keys.

use std::path::Path;

use mergus_contracts::hash::sha256_hex;
use mergus_ports::FileSystem;
use serde_json::Value;

use super::run::{orders, run_on_duckdb};
use super::{PROJECT, ROOT, exists, mergus, project, text};

#[test]
fn mg_req_1313_same_key_has_no_second_effect() {
    let files = project(&[("project.toml", PROJECT)]);
    let first = mergus(
        &files,
        &["new", "model", "orders", "--idempotency-key", "k1"],
    );
    assert_eq!(first.exit, 0, "{first:?}");
    assert!(exists(&files, "models/orders.sql"));
    let record = format!(".mergus/idempotency/{}.json", sha256_hex(b"k1"));
    let stored: Value = serde_json::from_str(&text(&files, &record).unwrap()).unwrap();
    assert_eq!(stored["exit_code"], 0);

    // The same key and arguments: the first result again, and no new file.
    files
        .remove_file(&Path::new(ROOT).join("models/orders.sql"))
        .unwrap();
    let second = mergus(
        &files,
        &["new", "model", "orders", "--idempotency-key", "k1"],
    );
    assert_eq!(second, first);
    assert!(!exists(&files, "models/orders.sql"));

    // The same key with other arguments fails and does nothing.
    let other = mergus(
        &files,
        &["new", "model", "customers", "--idempotency-key", "k1"],
    );
    assert_eq!(other.exit, 1, "{other:?}");
    assert!(other.stderr.contains("MG-1314"), "{other:?}");
    assert!(!exists(&files, "models/customers.sql"));
}

#[test]
fn mg_req_1313_run_replays_without_running() {
    let files = orders();
    let first = run_on_duckdb(&files, &["run", "--json", "--idempotency-key", "r1"]);
    assert_eq!(first.exit, 0, "{first:?}");
    let candidates = Path::new(ROOT).join(".mergus/work/candidates");
    files.remove_dir_all(&candidates).unwrap();
    let second = run_on_duckdb(&files, &["run", "--json", "--idempotency-key", "r1"]);
    assert_eq!(second, first);
    assert!(!files.exists(&candidates).unwrap());
}

#[test]
fn mg_req_1313_key_expires_after_24_hours() {
    let files = project(&[("project.toml", PROJECT)]);
    let args = ["new", "model", "orders", "--idempotency-key", "k2"];
    assert_eq!(mergus(&files, &args).exit, 0);
    let record = Path::new(ROOT).join(format!(".mergus/idempotency/{}.json", sha256_hex(b"k2")));
    let mut stored: Value = serde_json::from_slice(&files.read(&record).unwrap()).unwrap();
    let created = stored["created_unix_seconds"].as_i64().unwrap();
    stored["created_unix_seconds"] = Value::from(created - 24 * 3600 - 1);
    files.write(&record, stored.to_string().as_bytes()).unwrap();
    files
        .remove_file(&Path::new(ROOT).join("models/orders.sql"))
        .unwrap();

    // An expired key is a new key: the command runs again.
    assert_eq!(mergus(&files, &args).exit, 0);
    assert!(exists(&files, "models/orders.sql"));
}
