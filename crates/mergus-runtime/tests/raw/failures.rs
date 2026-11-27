//! Tests of failed loads in `ops.loads` (MG-REQ-0514.1, MG-REQ-0517.1) and of the order of
//! its rows (MG-REQ-0514.2).

use mergus_adapter::VersionRef;
use mergus_ports::Timestamp;
use mergus_runtime::{
    LoadMode, LoadRecord, LoadResult, last_watermark, load_raw_off_failed, record_load,
};

use super::memory::Memory;
use super::registry::{clock_at, incremental, marks, optional_texts};
use super::{FIRST_AT, SECOND_AT, ints, sales, texts};

#[cfg(test)]
/// A successful row of `ops.loads` for `erp.sales`, started at `at` microseconds.
fn row(id: &str, at: i64, after: &str) -> LoadRecord {
    LoadRecord {
        load_id: id.to_owned(),
        source: "erp".to_owned(),
        table: "sales".to_owned(),
        mode: LoadMode::Incremental,
        rows: 1,
        watermark_before: None,
        watermark_after: Some(after.to_owned()),
        started_at: Timestamp::from_unix_nanos(i128::from(at) * 1_000),
        duration_ms: 0,
        result: LoadResult::Ok,
        error_code: None,
    }
}

#[test]
fn mg_req_0514_last_watermark_ignores_read_order() {
    // Rows written newest first: the last load is the greatest `started_at`, then `load_id`,
    // not the last row read.
    let mut ops = Memory::default();
    for record in [
        row("0b", SECOND_AT, "newest"),
        row("0a", SECOND_AT, "same time, smaller id"),
        row("0c", FIRST_AT, "oldest"),
    ] {
        record_load(&mut ops, &record).unwrap();
    }
    let version = VersionRef::new(ops.commits().to_string());
    let mark = last_watermark(&ops, &version, "erp", "sales").unwrap();
    assert_eq!(mark.as_deref(), Some("newest"));
}

#[test]
fn mg_req_0517_failed_raw_off_is_recorded() {
    let mut ops = Memory::default();
    let extraction = incremental("l1", FIRST_AT, sales(&[(1, 10)]));
    let clock = clock_at(FIRST_AT);
    let read = marks(Some("2025-12-31 00:00:00"), Some("ignored"));
    load_raw_off_failed(&mut ops, &clock, &extraction, &read, "MG-0506").unwrap();
    let loads = ops.latest("loads");
    assert_eq!(texts(&loads, "mode"), ["raw_off"]);
    assert_eq!(texts(&loads, "result"), ["failed"]);
    assert_eq!(ints(&loads, "rows"), [0]);
    assert_eq!(
        optional_texts(&loads, "watermark_after"),
        [Some("2025-12-31 00:00:00".to_owned())]
    );
    assert_eq!(
        optional_texts(&loads, "error_code"),
        [Some("MG-0506".to_owned())]
    );
}
