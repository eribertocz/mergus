//! Tests of the load registry `ops.loads` (MG-REQ-0514) and of sources without raw
//! (MG-REQ-0517).

use arrow::array::{AsArray, RecordBatch};
use arrow::datatypes::{DataType, Int64Type};
use mergus_adapter::VersionRef;
use mergus_ports::Timestamp;
use mergus_runtime::{
    RawError, RawLoad, Watermarks, last_watermark, load_raw, load_raw_off, new_load_id,
};
use mergus_sim::{ClockFaults, SimClock, SimRandom};
use serde_json::json;

use super::memory::Memory;
use super::retention::configuration;
use super::{FIRST_AT, SECOND_AT, ints, load, sales, texts, times, utc};

/// 1.25 s, in microseconds: how long each load of these tests takes.
const TOOK: i64 = 1_250_000;

#[cfg(test)]
/// A clock at `micros` microseconds since the epoch.
pub(super) fn clock_at(micros: i64) -> SimClock {
    SimClock::new(
        1,
        Timestamp::from_unix_nanos(i128::from(micros) * 1_000),
        ClockFaults::default(),
    )
}

#[cfg(test)]
pub(super) fn marks(before: Option<&str>, after: Option<&str>) -> Watermarks {
    Watermarks {
        before: before.map(str::to_owned),
        after: after.map(str::to_owned),
        deletes_checked_at: None,
    }
}

#[cfg(test)]
/// A load of the incremental table `sales`, keyed by `sale_id`.
pub(super) fn incremental(id: &str, at: i64, rows: RecordBatch) -> RawLoad {
    let mut load = load(&["sale_id"], id, at, rows, Vec::new());
    load.table.incremental = Some("sold_at".to_owned());
    load
}

#[cfg(test)]
pub(super) fn optional_texts(batch: &RecordBatch, column: &str) -> Vec<Option<String>> {
    let column = batch.column_by_name(column).unwrap().as_string::<i32>();
    column
        .iter()
        .map(|value| value.map(str::to_owned))
        .collect()
}

#[test]
fn mg_req_0514_every_load_registered() {
    let (mut raw, mut ops) = (Memory::default(), Memory::default());
    let first = incremental("l1", FIRST_AT, sales(&[(1, 10), (2, 20)]));
    let clock = clock_at(FIRST_AT + TOOK);
    let after_first = marks(None, Some("2026-01-01 00:00:00.000002"));
    let version = load_raw(&mut raw, &mut ops, &clock, &first, None, &after_first).unwrap();
    let second = incremental("l2", SECOND_AT, sales(&[(3, 30)]));
    let clock = clock_at(SECOND_AT + TOOK);
    let after_second = marks(
        Some("2026-01-01 00:00:00.000002"),
        Some("2026-01-01 00:00:00.000003"),
    );
    let version = load_raw(
        &mut raw,
        &mut ops,
        &clock,
        &second,
        Some(&version),
        &after_second,
    )
    .unwrap();

    // MG-REQ-0514.1: a load refused with MG-0504 is recorded as failed, with 0 rows and its
    // watermark unchanged.
    let twice = incremental("l3", SECOND_AT, sales(&[(5, 1), (5, 2)]));
    let refused = marks(Some("2026-01-01 00:00:00.000003"), Some("ignored"));
    let error = load_raw(&mut raw, &mut ops, &clock, &twice, Some(&version), &refused);
    assert!(
        matches!(error, Err(RawError::RepeatedKey { .. })),
        "{error:?}"
    );
    assert_eq!(raw.commits(), 2);
    assert_eq!(ops.commits(), 3);
    assert_three_loads(&ops.latest("loads"));
    registry_after_raw(&mut raw, &mut ops, &version);
}

/// The rows of `ops.loads` after a full load, an incremental one and one refused with
/// MG-0504.
#[cfg(test)]
fn assert_three_loads(loads: &RecordBatch) {
    assert_loads_schema(loads);
    assert_eq!(texts(loads, "load_id"), ["l1", "l2", "l3"]);
    assert_eq!(texts(loads, "source"), ["erp", "erp", "erp"]);
    assert_eq!(texts(loads, "table"), ["sales", "sales", "sales"]);
    assert_eq!(texts(loads, "mode"), ["full", "incremental", "incremental"]);
    assert_eq!(ints(loads, "rows"), [2, 1, 0]);
    assert_eq!(
        optional_texts(loads, "watermark_before"),
        [
            None,
            Some("2026-01-01 00:00:00.000002".to_owned()),
            Some("2026-01-01 00:00:00.000003".to_owned())
        ]
    );
    assert_eq!(
        optional_texts(loads, "watermark_after"),
        [
            Some("2026-01-01 00:00:00.000002".to_owned()),
            Some("2026-01-01 00:00:00.000003".to_owned()),
            Some("2026-01-01 00:00:00.000003".to_owned())
        ]
    );
    assert_eq!(
        times(loads, "started_at"),
        [Some(FIRST_AT), Some(SECOND_AT), Some(SECOND_AT)]
    );
    assert_eq!(ints(loads, "duration_ms"), [1_250, 1_250, 1_250]);
    assert_eq!(texts(loads, "result"), ["ok", "ok", "failed"]);
    assert_eq!(
        optional_texts(loads, "error_code"),
        [None, None, Some("MG-0504".to_owned())]
    );
}

/// MG-REQ-0514: the columns of `ops.loads`, in order, with their types; `deletes_checked_at`
/// is MG-REQ-0415.3's.
#[cfg(test)]
fn assert_loads_schema(loads: &RecordBatch) {
    let schema = loads.schema();
    let columns: Vec<(&str, DataType)> = schema
        .fields()
        .iter()
        .map(|field| (field.name().as_str(), field.data_type().clone()))
        .collect();
    assert_eq!(
        columns,
        [
            ("load_id", DataType::Utf8),
            ("source", DataType::Utf8),
            ("table", DataType::Utf8),
            ("mode", DataType::Utf8),
            ("rows", DataType::Int64),
            ("watermark_before", DataType::Utf8),
            ("watermark_after", DataType::Utf8),
            ("started_at", utc()),
            ("duration_ms", DataType::Int64),
            ("result", DataType::Utf8),
            ("error_code", DataType::Utf8),
            ("deletes_checked_at", utc()),
        ]
    );
}

/// MG-REQ-0514.2: the registry row is committed after the raw commit; a raw version without
/// a row is ignored, and the next load starts from the last recorded `watermark_after`.
#[cfg(test)]
fn registry_after_raw(raw: &mut Memory, ops: &mut Memory, version: &VersionRef) {
    let recorded = VersionRef::new(ops.commits().to_string());
    let mark = last_watermark(ops, &recorded, "erp", "sales").unwrap();
    assert_eq!(mark.as_deref(), Some("2026-01-01 00:00:00.000003"));

    ops.fail_commits = true;
    let lost = incremental("l4", SECOND_AT, sales(&[(6, 60)]));
    let next = marks(
        Some("2026-01-01 00:00:00.000003"),
        Some("2026-01-01 00:00:00.000006"),
    );
    let clock = clock_at(SECOND_AT + TOOK);
    let error = load_raw(raw, ops, &clock, &lost, Some(version), &next).unwrap_err();
    assert!(matches!(error, RawError::Registry(_)), "{error:?}");
    // Raw was committed first; the registry did not record it.
    assert_eq!(raw.commits(), 3);
    assert_eq!(ops.commits(), 3);
    let mark = last_watermark(ops, &recorded, "erp", "sales").unwrap();
    assert_eq!(mark.as_deref(), Some("2026-01-01 00:00:00.000003"));
    assert_eq!(
        last_watermark(ops, &recorded, "erp", "other").unwrap(),
        None
    );
}

#[test]
fn mg_req_0514_load_id_is_a_uuid_v7() {
    let clock = clock_at(FIRST_AT);
    let random = SimRandom::new(3);
    let id = new_load_id(&clock, &random);
    assert_eq!(id.len(), 36, "{id}");
    let parts: Vec<&str> = id.split('-').collect();
    assert_eq!(
        parts.iter().map(|part| part.len()).collect::<Vec<_>>(),
        [8, 4, 4, 4, 12]
    );
    assert!(
        id.chars()
            .all(|c| c == '-' || c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "{id}"
    );
    // The first 48 bits are the milliseconds since the epoch, then version 7 and variant 10.
    let millis = u64::try_from(FIRST_AT / 1_000).unwrap();
    assert_eq!(
        format!("{}{}", parts[0], parts[1]),
        format!("{millis:012x}")
    );
    assert!(parts[2].starts_with('7'), "{id}");
    assert!(matches!(&parts[3][..1], "8" | "9" | "a" | "b"), "{id}");
    assert_ne!(new_load_id(&clock, &random), id);
}

#[test]
fn mg_req_0517_raw_off_is_recorded() {
    let mut ops = Memory::default();
    let extraction = incremental("l1", FIRST_AT, sales(&[(1, 10), (2, 20), (3, 30)]));
    let clock = clock_at(FIRST_AT + TOOK);
    let read = marks(
        Some("2025-12-31 00:00:00"),
        Some("2026-01-01 00:00:00.000003"),
    );
    // MG-REQ-0517.1: only the registry is written; there is no raw adapter to write to.
    load_raw_off(&mut ops, &clock, &extraction, &read).unwrap();
    assert_eq!(ops.commits(), 1);
    assert_eq!(ops.writes.len(), 1);
    let loads = ops.latest("loads");
    assert_eq!(texts(&loads, "mode"), ["raw_off"]);
    assert_eq!(ints(&loads, "rows"), [3]);
    assert_eq!(texts(&loads, "result"), ["ok"]);
    assert_eq!(
        optional_texts(&loads, "watermark_after"),
        [Some("2026-01-01 00:00:00.000003".to_owned())]
    );
    assert_eq!(
        loads
            .column_by_name("duration_ms")
            .unwrap()
            .as_primitive::<Int64Type>()
            .value(0),
        1_250
    );
    assert!(flags_absent(&loads));

    // The choice is read from each source: only `raw = false` turns raw off.
    let configuration = configuration(&[
        ("crm", json!({"kind": "database", "raw": false})),
        ("erp", json!({"kind": "database", "raw": true})),
        ("web", json!({"kind": "files", "location": "data"})),
    ]);
    assert_eq!(mergus_runtime::raw_off_sources(&configuration), ["crm"]);
}

/// `ops.loads` carries no raw metadata column.
#[cfg(test)]
fn flags_absent(loads: &RecordBatch) -> bool {
    loads.column_by_name("_deleted").is_none() && loads.column_by_name("_load_id").is_none()
}

#[test]
fn mg_req_0514_rows_of_every_batch_are_counted() {
    let mut ops = Memory::default();
    let mut extraction = load(&[], "l1", FIRST_AT, sales(&[(1, 10)]), Vec::new());
    extraction.rows.push(sales(&[(2, 20), (3, 30)]));
    let clock = clock_at(FIRST_AT);
    load_raw_off(&mut ops, &clock, &extraction, &marks(None, None)).unwrap();
    let loads = ops.latest("loads");
    assert_eq!(ints(&loads, "rows"), [3]);
    assert_eq!(texts(&loads, "mode"), ["raw_off"]);
    assert_eq!(optional_texts(&loads, "watermark_before"), [None]);
}
