//! MG-REQ-0418 and MG-REQ-0143: an interrupted load resumes from its last committed chunk.

use mergus_source::DEFAULT_OVERLAP;
use serde_json::json;

use crate::ALL;
use crate::fixture::{Row, attach, declared, row, source};
use crate::world::{World, flags, texts, versions};

#[cfg(test)]
fn rows(count: i64, hour: i64) -> Vec<Row> {
    (1..=count).map(|id| row(id, id * 10, Some(hour))).collect()
}

#[test]
fn mg_req_0418_extraction_resumes_from_last_chunk() {
    let source = source(json!({ "deletes": "none" }));
    let table = declared(&source);
    let mut world = World::new();
    let data = rows(5, 10);
    // Chunks of 2 rows: the process crashes after the first two chunks are committed.
    let mut attached = attach(&data, Some(2), DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, Some(2), Some(2))
        .unwrap_err();
    assert_eq!(world.raw.at("erp__sales__changes", 2).num_rows(), 4);
    let unfinished = world.every_change();
    assert_eq!(versions(&unfinished).len(), 4);
    let load_id = texts(&unfinished, "_load_id")[0].clone().unwrap();
    // Readers ignore the unfinished load (MG-REQ-0518.1), which is not recorded as failed.
    assert_eq!(world.changes(&table).num_rows(), 0);
    assert_eq!(world.ops.commits(), 0);

    // The next extraction resumes it with the same load id, after its largest key, 4.
    let mut attached = attach(&data, Some(2), DEFAULT_OVERLAP, None, &ALL);
    let outcome = world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap();
    assert!(outcome.resumed);
    assert_eq!(outcome.load_id, load_id);
    let changes = world.changes(&table);
    assert_eq!(versions(&changes), versions(&world.current()));
    assert_eq!(
        versions(&changes),
        [
            (1, 10, false),
            (2, 20, false),
            (3, 30, false),
            (4, 40, false),
            (5, 50, false)
        ]
    );
    let ids = texts(&changes, "_load_id");
    assert!(ids.iter().all(|id| id.as_deref() == Some(load_id.as_str())));
    let extracted = texts(&changes, "_extracted_at");
    assert!(extracted.iter().all(|at| at == &extracted[0]));
    assert_eq!(texts(&world.registry(), "result"), [Some("ok".to_owned())]);
}

#[test]
fn mg_req_0418_table_without_key_reads_again() {
    let source = source(json!({ "key": null, "incremental": null }));
    let table = declared(&source);
    let mut world = World::new();
    let data = rows(3, 10);
    // The read is one chunk; the crash comes before the last commit of raw.
    let mut attached = attach(&data, None, DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, None, Some(1))
        .unwrap_err();
    let mut attached = attach(&data, None, DEFAULT_OVERLAP, None, &ALL);
    let outcome = world
        .load(&mut attached, &source, &table, None, None)
        .unwrap();
    assert!(!outcome.resumed);
    // The unfinished rows stay in `__changes`, ignored by every reader (MG-REQ-0418.3).
    assert_eq!(world.every_change().num_rows(), 6);
    let changes = world.changes(&table);
    assert_eq!(versions(&changes), versions(&world.current()));
    assert_eq!(changes.num_rows(), 3);
}

#[cfg(test)]
/// The source of the exactly-once scenario: chunks of 2, a compare at every run.
fn scenario() -> (mergus_project::Source, [Vec<Row>; 2]) {
    let source = source(json!({ "deletes": "column:removed", "delete_check": "every_run" }));
    let first = rows(5, 10);
    let mut second: Vec<Row> = rows(7, 11);
    // Row 1 is unchanged inside the overlap, row 3 is marked removed, row 5 is gone.
    second[0] = row(1, 10, Some(10));
    second[2].removed = true;
    second.remove(4);
    (source, [first, second])
}

#[cfg(test)]
/// Runs both loads of the scenario, the second crashing after `crash` commits and then
/// resumed by a new process.
fn run(crash: Option<usize>) -> World {
    let (source, loads) = scenario();
    let table = declared(&source);
    let mut world = World::new();
    let mut attached = attach(&loads[0], Some(2), DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap();
    if let Some(crash) = crash {
        let mut attached = attach(&loads[1], Some(2), DEFAULT_OVERLAP, None, &ALL);
        world
            .load(&mut attached, &source, &table, Some(2), Some(crash))
            .unwrap_err();
    }
    let mut attached = attach(&loads[1], Some(2), DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap();
    world
}

#[test]
fn mg_req_0143_resume_is_exactly_once() {
    let whole = run(None);
    let (source, _) = scenario();
    let table = declared(&source);
    let expected = (versions(&whole.changes(&table)), versions(&whole.current()));
    // Each load commits 3 chunks, the last commit of raw and its `ops.loads` row; the
    // second load's last chunk is empty and commits nothing.
    assert_eq!(whole.raw.commits() + whole.ops.commits(), 10);
    for crash in 0..5 {
        let world = run(Some(crash));
        let changes = world.changes(&table);
        assert_eq!(
            (versions(&changes), versions(&world.current())),
            expected,
            "crash after {crash} commits of the second load"
        );
        assert!(
            flags(&world.current(), "_deleted")
                .iter()
                .all(|deleted| !deleted)
        );
        assert_eq!(texts(&world.registry(), "result").len(), 2);
    }
}
