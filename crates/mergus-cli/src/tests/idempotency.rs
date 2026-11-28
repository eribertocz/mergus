//! MG-REQ-1313: idempotency keys.

use std::io::Write;
use std::path::Path;

use mergus_contracts::hash::sha256_hex;
use mergus_ports::FileSystem;
use mergus_ports::real::RealDatabase;
use mergus_sim::{SimFileSystem, SimRandom};
use serde_json::{Value, json};

use super::run::{orders, run_on_duckdb};
use super::{PROJECT, ROOT, START, Setup, exists, mergus, project, text};
use crate::idempotency::Tee;

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

    claimed_key_has_no_effect();
    cancelled_call_runs_again();
}

/// The claim of a call still running, made `age` seconds before the simulated start.
fn claim(files: &SimFileSystem, key: &[u8], age: i64) -> std::path::PathBuf {
    let now = i64::try_from(START / 1_000_000_000).unwrap();
    let claim = json!({
        "arguments": "0".repeat(64),
        "created_unix_seconds": now - age,
        "state": "running",
    });
    let path = Path::new(ROOT).join(format!(".mergus/idempotency/{}.json", sha256_hex(key)));
    files.create_dir_all(path.parent().unwrap()).unwrap();
    files.write(&path, claim.to_string().as_bytes()).unwrap();
    path
}

/// MG-REQ-1313.2: a call that finds its key claimed by a call still running fails with
/// MG-1315, exit code 3, and does nothing; a claim older than 24 hours has expired.
fn claimed_key_has_no_effect() {
    let files = project(&[("project.toml", PROJECT)]);
    let args = ["new", "model", "orders", "--idempotency-key", "c1"];
    let path = claim(&files, b"c1", 60);
    let before = files.read(&path).unwrap();
    let claimed = mergus(&files, &args);
    assert_eq!(claimed.exit, 3, "{claimed:?}");
    assert!(claimed.stderr.contains("MG-1315"), "{claimed:?}");
    assert!(!exists(&files, "models/orders.sql"));
    // The claim is the running call's: it is left as it was.
    assert_eq!(files.read(&path).unwrap(), before);

    // A stale claim counts as expired: the command runs and keeps its result.
    claim(&files, b"c1", 24 * 3600 + 1);
    let stale = mergus(&files, &args);
    assert_eq!(stale.exit, 0, "{stale:?}");
    assert!(exists(&files, "models/orders.sql"));
    let stored: Value = serde_json::from_slice(&files.read(&path).unwrap()).unwrap();
    assert_eq!(stored["exit_code"], 0);
}

/// MG-REQ-1313.3: a cancelled call, exit code 7, keeps no result, so its retry runs again.
fn cancelled_call_runs_again() {
    let files = orders();
    let args = ["run", "--json", "--idempotency-key", "x1"];
    let database = RealDatabase::new(Path::new("/nonexistent"));
    let random = SimRandom::new(7);
    let cancelled = Setup {
        files: &files,
        cwd: ROOT,
        args: &args,
        vars: &[],
        random: &random,
        seed: 7,
        terminal: false,
        input: None,
    }
    .run_cancelled(&database);
    assert_eq!(cancelled.exit, 7, "{cancelled:?}");
    let record = format!(".mergus/idempotency/{}.json", sha256_hex(b"x1"));
    assert!(!exists(&files, &record), "{cancelled:?}");

    let retried = run_on_duckdb(&files, &args);
    assert_eq!(retried.exit, 0, "{retried:?}");
    assert_eq!(retried.of("run_finished")[0]["exit_code"], 0);
    let stored: Value = serde_json::from_str(&text(&files, &record).unwrap()).unwrap();
    assert_eq!(stored["exit_code"], 0);
}

#[test]
fn mg_req_1313_record_is_written_whole_or_not_at_all() {
    let files = project(&[("project.toml", PROJECT)]);
    let record = format!(".mergus/idempotency/{}.json", sha256_hex(b"k1"));
    // The process dies just before the record is moved into place: no record, not a
    // truncated one, is left.
    files.crash_before_next_rename();
    let crashed = mergus(
        &files,
        &["new", "model", "orders", "--idempotency-key", "k1"],
    );
    assert!(exists(&files, "models/orders.sql"), "{crashed:?}");
    assert!(!exists(&files, &record), "{crashed:?}");

    let kept = mergus(
        &files,
        &["new", "model", "customers", "--idempotency-key", "k2"],
    );
    assert_eq!(kept.exit, 0, "{kept:?}");
    let record = format!(".mergus/idempotency/{}.json", sha256_hex(b"k2"));
    let stored: Value = serde_json::from_str(&text(&files, &record).unwrap()).unwrap();
    assert_eq!(stored["exit_code"], 0);
}

#[test]
fn mg_req_1313_run_streams_its_events_with_a_key() {
    // Each write reaches the output at once, and is kept for the record.
    let mut out = Vec::new();
    let mut tee = Tee::new(&mut out);
    tee.write_all(b"{\"type\":\"run_started\"}\n").unwrap();
    tee.write_all(b"{\"type\":\"plan_ready\"}\n").unwrap();
    let copy = tee.copy;
    assert_eq!(out, copy);
    assert_eq!(out.iter().filter(|byte| **byte == b'\n').count(), 2);

    let files = orders();
    let keyed = run_on_duckdb(&files, &["run", "--json", "--idempotency-key", "s1"]);
    assert_eq!(keyed.exit, 0, "{keyed:?}");
    // The events were written as the run went: the output is the stored one, whole.
    let record = format!(".mergus/idempotency/{}.json", sha256_hex(b"s1"));
    let stored: Value = serde_json::from_str(&text(&files, &record).unwrap()).unwrap();
    assert_eq!(stored["stdout"], keyed.stdout);
    assert_eq!(keyed.of("run_finished").len(), 1);
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
