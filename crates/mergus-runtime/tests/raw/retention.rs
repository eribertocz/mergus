//! Tests of the immutable versions of raw (MG-REQ-0513) and of its retention (MG-REQ-0516).

use std::time::Duration;

use mergus_adapter::{Retention, VersionRef, WriteMode};
use mergus_project::{ConfigFile, Configuration, Source};
use mergus_runtime::{
    DEFAULT_RETENTION, expire_raw, load_raw, raw_retention, show_retention, source_retention,
    write_raw,
};
use serde_json::{Value, json};

use super::memory::Memory;
use super::registry::clock_at;
use super::{FIRST_AT, SECOND_AT, ints, load, sales};

const DAY: Duration = Duration::from_secs(86_400);

#[cfg(test)]
fn file(path: &str, value: Value) -> ConfigFile {
    ConfigFile {
        path: path.to_owned(),
        canonical: value.to_string(),
        value,
    }
}

#[cfg(test)]
/// A configuration with the sources `sources`, each a name and the JSON of its file.
pub(super) fn configuration(sources: &[(&str, Value)]) -> Configuration {
    Configuration {
        project: file("project.toml", json!({})),
        sources: sources
            .iter()
            .map(|(name, value)| Source {
                name: (*name).to_owned(),
                file: file(&format!("sources/{name}.toml"), value.clone()),
            })
            .collect(),
    }
}

#[test]
fn mg_req_0513_loads_are_immutable_versions() {
    let mut raw = Memory::default();
    let first = load(&["sale_id"], "l1", FIRST_AT, sales(&[(1, 10)]), Vec::new());
    let version = write_raw(&mut raw, &first, None).unwrap();
    let second = load(
        &["sale_id"],
        "l2",
        SECOND_AT,
        sales(&[(1, 15), (2, 20)]),
        Vec::new(),
    );
    write_raw(&mut raw, &second, Some(&version)).unwrap();
    let mut ops = Memory::default();
    let third = load(&["sale_id"], "l3", SECOND_AT, sales(&[(3, 30)]), Vec::new());
    let clock = clock_at(SECOND_AT);
    let marks = mergus_runtime::Watermarks {
        before: None,
        after: None,
        deletes_checked_at: None,
    };
    load_raw(
        &mut raw,
        &mut ops,
        &clock,
        &third,
        Some(&VersionRef::new("2")),
        &marks,
    )
    .unwrap();
    versions_unchanged(&raw);
    assert!(raw.expired.is_empty() && ops.expired.is_empty());
    expired_by_maintenance(&mut raw);
}

/// Each load is a version of its own; a later load leaves the earlier versions as they were.
#[cfg(test)]
fn versions_unchanged(raw: &Memory) {
    assert_eq!(raw.commits(), 3);
    assert_eq!(ints(&raw.at("erp__sales", 1), "sale_id"), [1]);
    assert_eq!(ints(&raw.at("erp__sales", 2), "sale_id"), [1, 2]);
    assert_eq!(ints(&raw.at("erp__sales__changes", 1), "sale_id"), [1]);
    assert_eq!(
        ints(&raw.at("erp__sales__changes", 2), "sale_id"),
        [1, 1, 2]
    );
    assert_eq!(
        ints(&raw.at("erp__sales__changes", 3), "sale_id"),
        [1, 1, 2, 3]
    );
    // A run only appends to `__changes` and replaces the current table; it never expires.
    assert!(raw.writes.iter().all(|(table, mode)| match mode {
        WriteMode::Append => table == "erp__sales__changes",
        _ => table == "erp__sales",
    }));
}

/// Versions are expired by maintenance only, with the default retention.
#[cfg(test)]
fn expired_by_maintenance(raw: &mut Memory) {
    let configuration = configuration(&[("erp", json!({"kind": "database"}))]);
    let live = vec![VersionRef::new("3")];
    expire_raw(raw, &configuration, live.clone()).unwrap();
    assert_eq!(
        raw.expired,
        [Retention {
            live,
            rollback_window: 30 * DAY
        }]
    );
}

#[test]
fn mg_req_0516_raw_retention_default_and_override() {
    // The default is 30 days of loads.
    assert_eq!(DEFAULT_RETENTION, 30 * DAY);
    let sources = configuration(&[
        ("crm", json!({"kind": "database", "retention": "45d"})),
        ("erp", json!({"kind": "database"})),
        (
            "hr",
            json!({"kind": "database", "raw": false, "retention": "90d"}),
        ),
        (
            "web",
            json!({"kind": "files", "location": "data", "retention": "12h"}),
        ),
    ]);
    let retention: Vec<Duration> = sources.sources.iter().map(source_retention).collect();
    assert_eq!(
        retention,
        [
            45 * DAY,
            30 * DAY,
            90 * DAY,
            Duration::from_secs(12 * 3_600)
        ]
    );

    // MG-REQ-0516.2: the shared catalog keeps the longest retention of the sources kept in raw.
    assert_eq!(raw_retention(&sources), Some(45 * DAY));
    let mut raw = Memory::default();
    expire_raw(&mut raw, &sources, Vec::new()).unwrap();
    assert_eq!(raw.expired[0].rollback_window, 45 * DAY);
    assert_eq!(
        raw_retention(&configuration(&[("erp", json!({"kind": "database"}))])),
        Some(30 * DAY)
    );
    let off = configuration(&[("hr", json!({"kind": "database", "raw": false}))]);
    assert_eq!(raw_retention(&off), None);

    // As `plan` shows it.
    assert_eq!(show_retention(45 * DAY), "45d");
    assert_eq!(show_retention(Duration::from_secs(12 * 3_600)), "12h");
    assert_eq!(show_retention(Duration::from_secs(90 * 60)), "90m");
}
