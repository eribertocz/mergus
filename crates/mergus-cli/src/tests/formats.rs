//! MG-REQ-0295 and MG-REQ-0296: the events and the formats of their common fields.

use mergus_ports::Random;
use mergus_sim::SimRandom;
use serde_json::Value;

use super::{PROJECT, ROOT, Setup, broken, mergus, project};

/// A random source that returns zeros first, then counts.
struct ZerosFirst(std::sync::Mutex<u64>);

impl Random for ZerosFirst {
    fn next_u64(&self) -> u64 {
        let mut draws = self.0.lock().unwrap();
        *draws += 1;
        if *draws <= 4 { 0 } else { *draws }
    }

    fn fill(&self, bytes: &mut [u8]) {
        for chunk in bytes.chunks_mut(8) {
            let draw = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&draw[..chunk.len()]);
        }
    }
}

fn run_id(random: &dyn Random) -> String {
    let files = broken();
    let outcome = Setup {
        files: &files,
        cwd: ROOT,
        args: &["validate", "--json"],
        vars: &[],
        random,
        seed: 7,
        terminal: false,
    }
    .run();
    let ids: Vec<Value> = outcome
        .events()
        .iter()
        .map(|e| e["run_id"].clone())
        .collect();
    assert!(
        ids.windows(2).all(|pair| pair[0] == pair[1]),
        "one run_id per command"
    );
    ids[0].as_str().unwrap().to_owned()
}

fn item_1_run_id() {
    let id = run_id(&SimRandom::new(1));
    assert_eq!(id.len(), 32);
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{id}"
    );
    assert_ne!(id, run_id(&SimRandom::new(2)), "drawn from the Random port");
    assert_eq!(id, run_id(&SimRandom::new(1)));
    let id = run_id(&ZerosFirst(std::sync::Mutex::new(0)));
    assert_ne!(id, "0".repeat(32), "never all zero");
}

fn item_2_sequence() {
    let events = mergus(&broken(), &["validate", "--json"]).events();
    assert!(events.len() >= 4);
    for (event, expected) in events.iter().zip(1_u64..) {
        assert_eq!(event["sequence"], expected);
    }
}

fn item_3_timestamp() {
    let events = mergus(&broken(), &["validate", "--json"]).events();
    for event in &events {
        assert_eq!(event["timestamp"], "2026-11-22T23:15:14.123Z", "{event}");
    }
}

fn item_4_location() {
    let files = project(&[
        ("project.toml", PROJECT),
        (
            "models/mergus.toml",
            "[orders]\ntags = [\"a\"]\n\n[ordres]\ntags = [\"b\"]\n",
        ),
        ("models/orders.sql", "select 1"),
    ]);
    let outcome = mergus(&files, &["validate", "--json"]);
    let diagnostics = outcome.of("diagnostic");
    assert_eq!(diagnostics.len(), 1, "{outcome:?}");
    let location = &diagnostics[0]["location"];
    assert_eq!(location["file"], "models/mergus.toml");
    assert_eq!(location["line"], 4);
    assert!(location.get("column").is_none());
    let unlocated = mergus(
        &project(&[("project.toml", PROJECT)]),
        &["valdate", "--json"],
    );
    assert!(unlocated.of("diagnostic")[0].get("location").is_none());
}

fn item_5_environment() {
    let outcome = mergus(
        &project(&[("project.toml", PROJECT)]),
        &["validate", "--json"],
    );
    assert_eq!(outcome.of("run_started")[0]["environment"], "prod");
}

#[test]
fn mg_req_0296_common_field_formats() {
    item_1_run_id();
    item_2_sequence();
    item_3_timestamp();
    item_4_location();
    item_5_environment();
}

#[test]
fn mg_req_0295_events_schema_and_required_fields() {
    let outcome = mergus(&broken(), &["validate", "--json"]);
    let started = &outcome.of("run_started")[0];
    assert_eq!(started["contract"], "mergus_events");
    assert_eq!(started["contract_version"], 1);
    assert_eq!(started["command"], "validate");
    assert_eq!(
        started["arguments"],
        serde_json::json!(["validate", "--json"])
    );
    let finished = &outcome.of("run_finished")[0];
    for field in ["status", "exit_code", "retryable", "duration_ms"] {
        assert!(finished.get(field).is_some(), "{field}: {finished}");
    }
    for diagnostic in outcome.of("diagnostic") {
        for field in ["code", "severity", "message", "hint"] {
            assert!(diagnostic.get(field).is_some(), "{field}: {diagnostic}");
        }
    }
}
