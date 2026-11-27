//! Tests of incremental loads by chunks, written through storage adapters in memory, from a
//! source answered by the simulated `Database` port (MG-REQ-0412, MG-REQ-0413, MG-REQ-0415,
//! MG-REQ-0418, MG-REQ-0420, MG-REQ-0143).

#[path = "../raw/memory.rs"]
mod memory;

mod deletes;
mod fixture;
mod resume;
mod world;

use std::time::Duration;

use mergus_source::DEFAULT_OVERLAP;
use serde_json::json;

use fixture::{at, attach, declared, row, source};
use world::{World, texts, versions};

const ALL: [&str; 4] = ["sale_id", "amount", "updated_at", "removed"];

#[test]
fn mg_req_0412_overlap_rereads_without_duplicates() {
    let source = source(json!({ "deletes": "none" }));
    let table = declared(&source);
    let mut world = first_load(&source, &table);
    // From `10:00 − 1h`: row 2 is re-read unchanged, row 3 changed, row 5 is new; row 1,
    // older, is not read. Only rows 3 and 5 are new versions (MG-REQ-0412.3).
    let second = [
        row(1, 10, Some(8)),
        row(2, 20, Some(9)),
        row(3, 31, Some(10)),
        row(5, 50, Some(11)),
    ];
    let mut attached = attach(&second, None, DEFAULT_OVERLAP, None, &ALL);
    let outcome = world
        .load(&mut attached, &source, &table, None, None)
        .unwrap();
    assert_eq!(outcome.rows, 2);
    assert_eq!(outcome.watermark, Some(at(11)));
    assert_eq!(
        versions(&world.changes(&table)),
        [
            (1, 10, false),
            (2, 20, false),
            (3, 30, false),
            (3, 31, false),
            (4, 40, false),
            (5, 50, false)
        ]
    );
    assert_eq!(
        versions(&world.current()),
        [
            (1, 10, false),
            (2, 20, false),
            (3, 31, false),
            (4, 40, false),
            (5, 50, false)
        ]
    );
    assert_registry(&world);
}

#[cfg(test)]
/// The first load of `sales`: a NULL incremental value is loaded and does not move the
/// watermark (MG-REQ-0412.1).
fn first_load(source: &mergus_project::Source, table: &mergus_source::DeclaredTable) -> World {
    let mut world = World::new();
    let first = [
        row(1, 10, Some(8)),
        row(2, 20, Some(9)),
        row(3, 30, Some(10)),
        row(4, 40, None),
    ];
    let mut attached = attach(&first, None, DEFAULT_OVERLAP, None, &ALL);
    let outcome = world
        .load(&mut attached, source, table, None, None)
        .unwrap();
    assert_eq!(outcome.watermark, Some(at(10)));
    world
}

#[cfg(test)]
/// MG-REQ-0413: the watermarks recorded in `ops.loads`.
fn assert_registry(world: &World) {
    let registry = world.registry();
    assert_eq!(texts(&registry, "watermark_before"), [None, Some(at(10))]);
    assert_eq!(
        texts(&registry, "watermark_after"),
        [Some(at(10)), Some(at(11))]
    );
    assert_eq!(
        texts(&registry, "mode"),
        [Some("full".to_owned()), Some("incremental".to_owned())]
    );
}

#[test]
fn mg_req_0413_watermark_after_commit() {
    let source = source(json!({ "deletes": "none" }));
    let table = declared(&source);
    let mut world = World::new();
    let first = [row(1, 10, Some(8))];
    let mut attached = attach(&first, Some(2), DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap();

    // Commits of the second load: 2 chunks, the last commit of raw, then `ops.loads`.
    let second = [
        row(1, 10, Some(8)),
        row(2, 20, Some(12)),
        row(3, 30, Some(13)),
    ];
    for crash in 0..4 {
        let mut copy = World::new();
        let mut attached = attach(&first, Some(2), DEFAULT_OVERLAP, None, &ALL);
        copy.load(&mut attached, &source, &table, Some(2), None)
            .unwrap();
        let mut attached = attach(&second, Some(2), DEFAULT_OVERLAP, None, &ALL);
        copy.load(&mut attached, &source, &table, Some(2), Some(crash))
            .unwrap_err();
        // Until its row of `ops.loads` is committed, the watermark stays where it was.
        let last = copy.loads().last.unwrap();
        assert_eq!(
            last.watermark_after,
            Some(at(8)),
            "crash after {crash} commits"
        );
    }
    let mut attached = attach(&second, Some(2), DEFAULT_OVERLAP, None, &ALL);
    world
        .load(&mut attached, &source, &table, Some(2), None)
        .unwrap();
    assert_eq!(world.loads().last.unwrap().watermark_after, Some(at(13)));
}

#[test]
fn mg_req_0420_very_late_rows_reported() {
    let source = source(json!({ "deletes": "compare", "delete_check": "every_run" }));
    let table = declared(&source);
    let mut world = World::new();
    let first = [row(1, 10, Some(10))];
    let mut attached = attach(&first, None, DEFAULT_OVERLAP, None, &ALL);
    assert_eq!(
        world
            .load(&mut attached, &source, &table, None, None)
            .unwrap()
            .late,
        None
    );

    // Rows 2 and 3 arrive with values older than `10:00 − 1h`: they are counted, not loaded.
    let second = [
        row(1, 10, Some(10)),
        row(2, 20, Some(7)),
        row(3, 30, Some(5)),
        row(4, 40, Some(11)),
    ];
    let mut attached = attach(&second, None, DEFAULT_OVERLAP, None, &ALL);
    let late = world
        .load(&mut attached, &source, &table, None, None)
        .unwrap()
        .late
        .unwrap();
    assert_eq!(
        (late.count, late.since.as_str(), late.oldest.as_str()),
        (2, "2026-01-01 09:00:00", "2026-01-01 05:00:00")
    );
    assert_eq!(late.code(), "MG-0411");
    assert_eq!(
        late.to_string(),
        "table erp.sales: 2 rows older than 2026-01-01 09:00:00 arrived late; run mergus \
         backfill --source erp --table sales --from 2026-01-01 05:00:00"
    );
    assert_eq!(versions(&world.current()), [(1, 10, false), (4, 40, false)]);
}

#[test]
fn mg_req_0420_overlap_covers_late_rows() {
    let first = [row(1, 10, Some(10))];
    let second = [
        row(1, 10, Some(10)),
        row(2, 20, Some(7)),
        row(3, 30, Some(5)),
        row(4, 40, Some(11)),
    ];
    // A longer overlap reads them: nothing is late.
    let wide = source_with_overlap("6h");
    let table = declared(&wide);
    let mut world = World::new();
    let mut attached = attach(&first, None, Duration::from_secs(6 * 3_600), None, &ALL);
    world
        .load(&mut attached, &wide, &table, None, None)
        .unwrap();
    let mut attached = attach(&second, None, Duration::from_secs(6 * 3_600), None, &ALL);
    let outcome = world
        .load(&mut attached, &wide, &table, None, None)
        .unwrap();
    assert_eq!(outcome.late, None);
    assert_eq!(versions(&world.current()).len(), 4);
}

#[cfg(test)]
fn source_with_overlap(overlap: &str) -> mergus_project::Source {
    source(json!({ "deletes": "compare", "delete_check": "every_run", "overlap": overlap }))
}
