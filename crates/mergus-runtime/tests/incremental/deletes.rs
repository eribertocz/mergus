//! MG-REQ-0415: the delete policies.

use std::time::Duration;

use mergus_runtime::LoadError;
use mergus_source::{DEFAULT_OVERLAP, ExtractError};
use serde_json::json;

use crate::ALL;
use crate::fixture::{DAY_ONE, HOUR, Row, attach, declared, row, source};
use crate::world::{World, texts, versions};

#[cfg(test)]
/// The current and `__changes` rows after loading `first` then `second` with `options`.
fn twice(options: serde_json::Value, first: &[Row], second: &[Row]) -> World {
    let source = source(options);
    let table = declared(&source);
    let mut world = World::new();
    for rows in [first, second] {
        let mut attached = attach(rows, None, DEFAULT_OVERLAP, None, &ALL);
        world
            .load(&mut attached, &source, &table, None, None)
            .unwrap();
    }
    world
}

#[test]
fn mg_req_0415_delete_policies() {
    let first = [
        row(1, 10, Some(10)),
        row(2, 20, Some(10)),
        row(3, 30, Some(10)),
    ];
    // Row 2 is gone from the source, row 3 is marked removed.
    let mut removed = row(3, 30, Some(11));
    removed.removed = true;
    let second = [row(1, 10, Some(10)), removed];

    // No `deletes`, or `none`: nothing is detected (MG-REQ-0415.1).
    for options in [json!({ "deletes": null }), json!({ "deletes": "none" })] {
        let world = twice(options, &first, &second);
        assert_eq!(
            versions(&world.current()),
            [(1, 10, false), (2, 20, false), (3, 30, false)]
        );
    }

    // `column:removed`: row 3 leaves the current table and is a delete of `__changes` with the
    // values read, the column kept (MG-REQ-0415.2).
    let world = twice(json!({ "deletes": "column:removed" }), &first, &second);
    assert_eq!(versions(&world.current()), [(1, 10, false), (2, 20, false)]);
    let source_ = source(json!({}));
    let changes = world.changes(&declared(&source_));
    assert_eq!(
        versions(&changes),
        [
            (1, 10, false),
            (2, 20, false),
            (3, 30, false),
            (3, 30, true)
        ]
    );
    let deleted_at = texts(&changes, "_deleted_at");
    let extracted_at = texts(&changes, "_extracted_at");
    assert_eq!(deleted_at[3], extracted_at[3]);
    assert_eq!(deleted_at[..3], [None, None, None]);
    assert_eq!(texts(&changes, "removed")[3].as_deref(), Some("true"));
}

#[test]
fn mg_req_0415_compare_marks_missing_keys() {
    let first = [
        row(1, 10, Some(10)),
        row(2, 20, Some(10)),
        row(3, 30, Some(10)),
    ];
    let mut removed = row(3, 30, Some(11));
    removed.removed = true;
    let second = [row(1, 10, Some(10)), removed];
    let source_ = source(json!({}));
    // `compare`: row 2, missing from the source, is a delete with its last known values
    // (MG-REQ-0415.3, MG-REQ-0518.2); row 3 is still in the source.
    let world = twice(
        json!({ "deletes": "compare", "delete_check": "every_run" }),
        &first,
        &second,
    );
    assert_eq!(versions(&world.current()), [(1, 10, false), (3, 30, false)]);
    let changes = world.changes(&declared(&source_));
    assert!(versions(&changes).contains(&(2, 20, true)));
    let registry = world.registry();
    let started = texts(&registry, "started_at");
    assert_eq!(texts(&registry, "deletes_checked_at"), started);
}

#[test]
fn mg_req_0415_compare_follows_delete_check() {
    let source = source(json!({ "deletes": "compare", "delete_check": "daily" }));
    let table = declared(&source);
    let mut world = World::new();
    let first = [row(1, 10, Some(10)), row(2, 20, Some(10))];
    let second = [row(1, 10, Some(10))];
    let mut attached = attach(&first, None, DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, None, None)
        .unwrap();
    // One minute later the daily compare is not due: row 2 stays current.
    let mut attached = attach(&second, None, DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, None, None)
        .unwrap();
    assert_eq!(versions(&world.current()), [(1, 10, false), (2, 20, false)]);
    let checked = texts(&world.registry(), "deletes_checked_at");
    assert_eq!(checked[0], checked[1]);
    // 24 hours after the first compare, it is due.
    world.clock.advance(Duration::from_secs(24 * 3_600));
    let mut attached = attach(&second, None, DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, None, None)
        .unwrap();
    assert_eq!(versions(&world.current()), [(1, 10, false)]);
}

#[test]
fn mg_req_0415_compare_within_delete_window() {
    let source =
        source(json!({ "deletes": "compare", "delete_check": "every_run", "delete_window": "6h" }));
    let table = declared(&source);
    let mut world = World::new();
    // Row 1 is older than `now − 6h`; row 2 is inside the window. Both leave the source.
    let first = [
        row(1, 10, Some(2)),
        row(2, 20, Some(10)),
        row(3, 30, Some(11)),
    ];
    let second = [row(3, 30, Some(11))];
    let now = DAY_ONE + 12 * HOUR;
    let window = Some((Duration::from_secs(6 * 3_600), now));
    let mut attached = attach(&first, None, DEFAULT_OVERLAP, window, &ALL);
    world
        .load(&mut attached, &source, &table, None, None)
        .unwrap();
    let second_now = now + 60_000_000;
    let window = Some((Duration::from_secs(6 * 3_600), second_now));
    let mut attached = attach(&second, None, DEFAULT_OVERLAP, window, &ALL);
    world
        .load(&mut attached, &source, &table, None, None)
        .unwrap();
    assert_eq!(versions(&world.current()), [(1, 10, false), (3, 30, false)]);
}

#[test]
fn mg_req_0415_missing_delete_column_fails() {
    let source = source(json!({ "deletes": "column:removed" }));
    let table = declared(&source);
    let mut world = World::new();
    let rows = [row(1, 10, Some(10))];
    let mut attached = attach(&rows, None, DEFAULT_OVERLAP, None, &ALL[..3]);
    let error = world
        .load(&mut attached, &source, &table, None, None)
        .unwrap_err();
    assert!(matches!(
        error,
        LoadError::Source(ExtractError::MissingDeleteColumn { .. })
    ));
    assert_eq!(error.code(), Some("MG-0406"));
    assert_eq!(
        error.to_string(),
        "table `sales` of source `erp` declares `deletes = \"column:removed\"` but the source \
         table has no column `removed`"
    );
    // Nothing is read; the load is recorded as failed with its code.
    assert_eq!(world.raw.commits(), 0);
    let registry = world.registry();
    assert_eq!(texts(&registry, "result"), [Some("failed".to_owned())]);
    assert_eq!(texts(&registry, "error_code"), [Some("MG-0406".to_owned())]);
}
