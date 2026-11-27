//! Tests of failed loads in `ops.loads` and their codes (MG-REQ-0514.1, MG-REQ-0517.1), and
//! of the order of its rows (MG-REQ-0514.2).

use std::path::Path;
use std::sync::Arc;

use mergus_adapter::VersionRef;
use mergus_ports::{Clock, Timestamp};
use mergus_runtime::{
    Catalog, Clients, LoadMode, LoadRecord, LoadResult, Profile, Zone, ZoneStorage, last_watermark,
    load_raw, load_raw_off_failed, open_raw, record_load,
};
use mergus_sim::{DatabaseFaults, SimDatabase};

use super::memory::Memory;
use super::registry::{clock_at, incremental, marks, optional_texts};
use super::{FIRST_AT, SECOND_AT, ints, load, sales, texts};

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

#[test]
fn mg_req_0514_every_failed_load_has_a_code() {
    // MG-0505: the rows lack a column of the key.
    let (mut raw, mut ops) = (Memory::default(), Memory::default());
    let mut rows = load(&["shop"], "l1", FIRST_AT, sales(&[(1, 10)]), Vec::new());
    rows.table.key = vec!["shop".to_owned()];
    let clock = clock_at(FIRST_AT);
    let error = load_raw(&mut raw, &mut ops, &clock, &rows, None, &marks(None, None));
    assert_eq!(error.unwrap_err().code(), Some("MG-0505"));

    // MG-0506: the raw storage refuses the commit.
    raw.fail_commits = true;
    let stored = load(&["sale_id"], "l2", FIRST_AT, sales(&[(1, 10)]), Vec::new());
    let error = load_raw(
        &mut raw,
        &mut ops,
        &clock,
        &stored,
        None,
        &marks(None, None),
    );
    assert_eq!(error.unwrap_err().code(), Some("MG-0506"));
    assert_eq!(
        optional_texts(&ops.latest("loads"), "error_code"),
        [Some("MG-0505".to_owned()), Some("MG-0506".to_owned())]
    );
    assert_eq!(texts(&ops.latest("loads"), "result"), ["failed", "failed"]);

    // MG-0507: a `cloud` catalog opened without the value of its secret.
    let storage = ZoneStorage {
        zone: Zone::Raw,
        profile: Profile::Cloud,
        location: "s3://lake/raw/".to_owned(),
        catalog: Catalog::Postgres {
            secret: "secret:raw_catalog".to_owned(),
        },
        data_path: "s3://lake/raw/".to_owned(),
        clients: Clients::Remote,
    };
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let clock: Arc<dyn Clock> = Arc::new(clock_at(FIRST_AT));
    let Err(error) = open_raw(&database, &storage, Path::new("/project"), None, clock) else {
        panic!("a cloud catalog without its connection was opened");
    };
    assert_eq!(error.code(), Some("MG-0507"));
}
