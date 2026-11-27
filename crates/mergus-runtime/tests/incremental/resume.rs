//! MG-REQ-0418 and MG-REQ-0143: an interrupted load resumes from its last committed chunk.

use mergus_source::DEFAULT_OVERLAP;
use serde_json::json;

use crate::ALL;
use mergus_ports::SqlValue;

use crate::fixture::{Row, attach, attach_with, declared, row, source};
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
/// The two loads of an exactly-once scenario, read in chunks of 2: row 1 is unchanged inside
/// the overlap, row 3 is marked removed, row 5 is gone and rows 6 and 7 are new.
fn loads() -> [Vec<Row>; 2] {
    let first = rows(5, 10);
    let mut second: Vec<Row> = rows(7, 11);
    second[0] = row(1, 10, Some(10));
    second[2].removed = true;
    second.remove(4);
    [first, second]
}

#[cfg(test)]
/// Runs both loads with `options`, the load `crash.0` crashing after `crash.1` commits and
/// then run again by a new process.
fn run(options: &serde_json::Value, crash: Option<(usize, usize)>) -> World {
    let source = source(options.clone());
    let table = declared(&source);
    let mut world = World::new();
    for (index, rows) in loads().iter().enumerate() {
        if let Some((_, after)) = crash.filter(|(load, _)| *load == index) {
            let mut attached = attach(rows, Some(2), DEFAULT_OVERLAP, None, &ALL);
            world
                .load(&mut attached, &source, &table, Some(2), Some(after))
                .unwrap_err();
        }
        let mut attached = attach(rows, Some(2), DEFAULT_OVERLAP, None, &ALL);
        world
            .load(&mut attached, &source, &table, Some(2), None)
            .unwrap();
    }
    world
}

/// One row of `__changes` or of the current table, its load told by its rank in `ops.loads`.
type Seen = (i64, i64, bool, usize, bool);

/// The rows of `__changes`, of the current table, and each load's watermark and compare.
type Snapshot = (Vec<Seen>, Vec<Seen>, Vec<(Option<String>, bool)>);

#[cfg(test)]
/// What readers see of `world`: every row with the rank of its load and whether
/// `_deleted_at` is its load's `_extracted_at` exactly for deletes, then each load's
/// `watermark_after` and whether it compared.
fn snapshot(world: &World) -> Snapshot {
    let registry = world.registry();
    let ids = texts(&registry, "load_id");
    let rank = |id: &Option<String>| ids.iter().position(|known| known == id).unwrap();
    let rows = |batch: &arrow::array::RecordBatch| -> Vec<Seen> {
        let loads = texts(batch, "_load_id");
        let (at, deleted_at) = (texts(batch, "_extracted_at"), texts(batch, "_deleted_at"));
        versions(batch)
            .into_iter()
            .enumerate()
            .map(|(row, (id, amount, deleted))| {
                let stamped = if deleted {
                    deleted_at[row] == at[row]
                } else {
                    deleted_at[row].is_none()
                };
                (id, amount, deleted, rank(&loads[row]), stamped)
            })
            .collect()
    };
    let table = declared(&source(json!({})));
    let marks = texts(&registry, "watermark_after")
        .into_iter()
        .zip(texts(&registry, "deletes_checked_at"))
        .map(|(mark, checked)| (mark, checked.is_some()))
        .collect();
    (rows(&world.changes(&table)), rows(&world.current()), marks)
}

#[test]
fn mg_req_0143_resume_is_exactly_once() {
    // The delete column, then a `compare` at every run.
    let policies = [
        json!({ "deletes": "column:removed" }),
        json!({ "deletes": "compare", "delete_check": "every_run" }),
    ];
    for options in &policies {
        let whole = run(options, None);
        let expected = snapshot(&whole);
        assert_eq!(texts(&whole.registry(), "result").len(), 2);
        assert!(
            flags(&whole.current(), "_deleted")
                .iter()
                .all(|deleted| !deleted)
        );
        // Each load commits its chunks, the last commit of raw and its `ops.loads` row.
        let total = whole.raw.commits() + whole.ops.commits();
        let firsts = commits_of_first(options);
        for (load, commits) in [(0, firsts), (1, total - firsts)] {
            for crash in 0..commits {
                let world = run(options, Some((load, crash)));
                assert_eq!(
                    snapshot(&world),
                    expected,
                    "{options}: load {load}, crash after {crash}"
                );
                assert_eq!(texts(&world.registry(), "result").len(), 2);
            }
        }
    }
}

#[cfg(test)]
/// The commits of the first load of the scenario, raw and ops together.
fn commits_of_first(options: &serde_json::Value) -> usize {
    let source = source(options.clone());
    let table = declared(&source);
    let mut world = World::new();
    let mut attached = attach(&loads()[0], Some(2), DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap();
    world.raw.commits() + world.ops.commits()
}

#[test]
fn mg_req_0418_last_commit_made_only_records() {
    let source = source(json!({ "deletes": "none" }));
    let table = declared(&source);
    let mut world = World::new();
    let data = rows(3, 10);
    // 2 chunks and the last commit of raw are made; the crash comes before `ops.loads`.
    let mut attached = attach(&data, Some(2), DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, Some(2), Some(3))
        .unwrap_err();
    assert_eq!((world.raw.commits(), world.ops.commits()), (3, 0));
    let mut attached = attach(&data, Some(2), DEFAULT_OVERLAP, None, &ALL);
    let outcome = world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap();
    // Only the registry row is written (MG-REQ-0418.2).
    assert!(outcome.resumed);
    assert_eq!((world.raw.commits(), world.ops.commits()), (3, 1));
    assert_eq!(outcome.watermark, Some(crate::fixture::at(10)));
    assert_eq!(versions(&world.changes(&table)).len(), 3);
}

#[test]
fn mg_req_0518_key_repeated_across_chunks_fails_the_load() {
    let source = source(json!({ "deletes": "none" }));
    let table = declared(&source);
    let mut world = World::new();
    let data = rows(3, 10);
    // The second chunk returns key 2 again, already read by the first.
    let after =
        "SELECT * FROM \"erp\".\"sales\" WHERE \"sale_id\" > ? ORDER BY \"sale_id\" LIMIT ?";
    let again = vec![crate::fixture::batch(&[
        row(2, 20, Some(10)),
        row(3, 30, Some(10)),
    ])];
    let bound = vec![SqlValue::Integer(2), SqlValue::Integer(2)];
    let first = vec![(after.to_owned(), bound, again)];
    let mut attached = attach_with(&data, Some(2), DEFAULT_OVERLAP, None, &ALL, first);
    let error = world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap_err();
    assert_eq!(error.code(), Some("MG-0504"));
    // The first chunk was committed, but the load is recorded `failed`: readers ignore it.
    assert_eq!(world.every_change().num_rows(), 2);
    let registry = world.registry();
    assert_eq!(texts(&registry, "result"), [Some("failed".to_owned())]);
    assert_eq!(texts(&registry, "error_code"), [Some("MG-0504".to_owned())]);
    assert_eq!(world.changes(&table).num_rows(), 0);
    // The next load is a new one, not a resume of the failed load.
    let mut attached = attach(&data, Some(2), DEFAULT_OVERLAP, None, &ALL);
    let outcome = world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap();
    assert!(!outcome.resumed);
    assert_eq!(versions(&world.changes(&table)).len(), 3);
}
