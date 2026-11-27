//! History tables on the real DuckDB: change detection (MG-REQ-0662), validity columns
//! (MG-REQ-0663), deletes (MG-REQ-0664), protection from rebuilds (MG-REQ-0665), the `as_of`
//! macro (MG-REQ-0666) and reading the changes view (MG-REQ-0667).

mod changes;
mod keys;

use std::path::PathBuf;

use arrow::array::{Array, AsArray};
use arrow::compute::cast;
use arrow::datatypes::DataType;
use mergus_engine_duckdb::{
    AS_OF, ChangeDetection, Deletes, EngineError, History, HistoryInput, RebuildHistory,
    apply_history, rebuild_history,
};
use mergus_ports::real::{RealDatabase, RealFileSystem, RealRandom};
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, FileSystem, Random, SqlValue, Timestamp,
};

/// 2026-03-01 00:00:00 UTC and 2026-04-01 00:00:00 UTC, in seconds.
const MARCH: i128 = 1_772_323_200;
const APRIL: i128 = 1_775_001_600;

/// An in-memory session of the real DuckDB.
#[cfg(test)]
fn session() -> Box<dyn DatabaseConnection> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "mergus-engine-duckdb-history-{:016x}",
        RealRandom::new().next_u64()
    ));
    RealFileSystem.create_dir_all(&dir).unwrap();
    RealDatabase::new(&dir.join("cache"))
        .connect(&DatabaseLocation::InMemory)
        .unwrap()
}

#[cfg(test)]
fn history(by: ChangeDetection, deletes: Deletes, input: HistoryInput, at: i128) -> History {
    History {
        table: "dim".to_owned(),
        input: "src".to_owned(),
        key: "id".to_owned(),
        by,
        deletes,
        input_kind: input,
        detected_at: Timestamp::from_unix_nanos(at * 1_000_000_000),
    }
}

#[cfg(test)]
fn segment_city() -> ChangeDetection {
    ChangeDetection::Differs(vec!["segment".to_owned(), "city".to_owned()])
}

/// Replaces the input `src` with `rows`, a `VALUES` list of `(id, updated_at, segment, city)`.
#[cfg(test)]
fn input(session: &mut dyn DatabaseConnection, values: &str) {
    session.execute("DROP TABLE IF EXISTS src", &[]).unwrap();
    session
        .execute(
            "CREATE TABLE src (id BIGINT, updated_at TIMESTAMP, segment VARCHAR, city VARCHAR)",
            &[],
        )
        .unwrap();
    if !values.is_empty() {
        let mut insert = "INSERT INTO src VALUES ".to_owned();
        insert.push_str(values);
        session.execute(&insert, &[]).unwrap();
    }
}

/// A version: id, segment, `_valid_from`, `_valid_to`, `_is_current`, `_is_deleted`.
type Version = (i64, Option<String>, String, String, bool, bool);

/// The versions of `dim`, by key and then `_valid_from`.
#[cfg(test)]
fn versions(session: &mut dyn DatabaseConnection) -> Vec<Version> {
    let sql = "SELECT id, segment, CAST(_valid_from AS VARCHAR), CAST(_valid_to AS VARCHAR), \
               _is_current, _is_deleted FROM dim ORDER BY id, _valid_from";
    let mut versions = Vec::new();
    for batch in session.query_arrow(sql, &[]).unwrap() {
        let id = cast(batch.column(0), &DataType::Int64).unwrap();
        let id = id.as_primitive::<arrow::datatypes::Int64Type>();
        let text = |column: usize, row: usize| {
            let array = batch.column(column).as_string::<i32>();
            (!array.is_null(row)).then(|| array.value(row).to_owned())
        };
        let flag = |column: usize, row: usize| batch.column(column).as_boolean().value(row);
        for row in 0..batch.num_rows() {
            versions.push((
                id.value(row),
                text(1, row),
                text(2, row).unwrap(),
                text(3, row).unwrap(),
                flag(4, row),
                flag(5, row),
            ));
        }
    }
    versions
}

const END: &str = "9999-12-31 00:00:00+00";
const ON_MARCH: &str = "2026-03-01 00:00:00+00";
const ON_APRIL: &str = "2026-04-01 00:00:00+00";

#[cfg(test)]
fn version(id: i64, segment: Option<&str>, from: &str, to: &str, flags: (bool, bool)) -> Version {
    (
        id,
        segment.map(str::to_owned),
        from.to_owned(),
        to.to_owned(),
        flags.0,
        flags.1,
    )
}

const CURRENT: (bool, bool) = (true, false);
const CLOSED: (bool, bool) = (false, false);
const TOMBSTONE: (bool, bool) = (true, true);

/// Applies a run of `by` with no deletes for each of `runs`: its input `VALUES` and its time.
#[cfg(test)]
fn apply_runs(session: &mut dyn DatabaseConnection, by: &ChangeDetection, runs: &[(&str, i128)]) {
    for (values, at) in runs {
        input(session, values);
        let run = history(by.clone(), Deletes::Ignore, HistoryInput::Other, *at);
        apply_history(session, &run).unwrap();
    }
}

/// `by = [columns]`: a new version when any column differs, NULL to NULL being no change.
#[cfg(test)]
fn list_mode() {
    let mut session = session();
    let runs = [
        ("(1, NULL, 'a', NULL), (2, NULL, 'b', 'y')", MARCH),
        ("(1, NULL, 'a', NULL), (2, NULL, 'b', 'w')", APRIL),
    ];
    apply_runs(session.as_mut(), &segment_city(), &runs);
    assert_eq!(
        versions(session.as_mut()),
        vec![
            version(1, Some("a"), ON_MARCH, END, CURRENT),
            version(2, Some("b"), ON_MARCH, ON_APRIL, CLOSED),
            version(2, Some("b"), ON_APRIL, END, CURRENT),
        ]
    );
}

/// `by = "<column>"`: a new version only when the value is strictly greater; an incoming NULL
/// never opens one, a value over a current NULL does.
#[cfg(test)]
fn column_mode() {
    let mut session = session();
    let runs = [
        (
            "(1, '2026-01-10', 'a', 'x'), (2, '2026-01-10', 'b', 'y'), (3, NULL, 'c', 'z')",
            MARCH,
        ),
        (
            "(1, '2026-01-20', 'a', 'x'), (2, '2026-01-10', 'B', 'y'), (3, '2026-01-15', 'c', 'z')",
            APRIL,
        ),
        (
            "(1, NULL, 'n', 'x'), (2, '2026-01-05', 'B', 'y'), (3, NULL, 'c', 'z')",
            APRIL,
        ),
    ];
    let by = ChangeDetection::Increases("updated_at".to_owned());
    apply_runs(session.as_mut(), &by, &runs);
    let (jan10, jan20) = ("2026-01-10 00:00:00+00", "2026-01-20 00:00:00+00");
    assert_eq!(
        versions(session.as_mut()),
        vec![
            version(1, Some("a"), jan10, jan20, CLOSED),
            version(1, Some("a"), jan20, END, CURRENT),
            version(2, Some("b"), jan10, END, CURRENT),
            // The first version took the detection time; the next `by` value, 2026-01-15, is
            // not later than it, so the new version takes the detection time too.
            version(3, Some("c"), ON_MARCH, ON_APRIL, CLOSED),
            version(3, Some("c"), ON_APRIL, END, CURRENT),
        ]
    );
}

#[test]
fn mg_req_0662_change_detection_modes() {
    list_mode();
    column_mode();
}

#[test]
fn mg_req_0663_by_column_must_be_a_time() {
    let mut session = session();
    session
        .execute(
            "CREATE TABLE src AS SELECT 1 AS id, CAST(7 AS BIGINT) AS version",
            &[],
        )
        .unwrap();
    let by = ChangeDetection::Increases("version".to_owned());
    let run = history(by, Deletes::Ignore, HistoryInput::Other, MARCH);
    let error = apply_history(session.as_mut(), &run).unwrap_err();
    assert!(matches!(
        &error,
        EngineError::HistoryByType { column, data_type, .. }
            if column == "version" && data_type == "BIGINT"
    ));
    assert_eq!(error.code(), Some("MG-0609"));
    assert!(session.query_arrow("SELECT * FROM dim", &[]).is_err());
}

#[test]
fn mg_req_0663_validity_columns() {
    let mut session = session();
    input(session.as_mut(), "(1, NULL, 'a', 'x')");
    let lists = history(segment_city(), Deletes::Ignore, HistoryInput::Other, MARCH);
    apply_history(session.as_mut(), &lists).unwrap();
    let described = session.describe("SELECT * FROM dim", &[]).unwrap();
    let columns: Vec<(&str, &str)> = described
        .iter()
        .map(|column| (column.name.as_str(), column.data_type.as_str()))
        .collect();
    assert_eq!(
        columns,
        vec![
            ("id", "BIGINT"),
            ("updated_at", "TIMESTAMP"),
            ("segment", "VARCHAR"),
            ("city", "VARCHAR"),
            ("_valid_from", "TIMESTAMP WITH TIME ZONE"),
            ("_valid_to", "TIMESTAMP WITH TIME ZONE"),
            ("_is_current", "BOOLEAN"),
            ("_is_deleted", "BOOLEAN"),
        ]
    );
    // Half-open validity: at the instant of a change, exactly one version of the key.
    input(session.as_mut(), "(1, NULL, 'b', 'x')");
    let april = history(segment_city(), Deletes::Ignore, HistoryInput::Other, APRIL);
    apply_history(session.as_mut(), &april).unwrap();
    let at = SqlValue::TimestampTz(i64::try_from(APRIL).unwrap() * 1_000_000);
    let count = "SELECT count(*) FROM dim WHERE _valid_from <= CAST(? AS TIMESTAMPTZ) \
                 AND CAST(? AS TIMESTAMPTZ) < _valid_to";
    let found = session.query_arrow(count, &[at.clone(), at]).unwrap();
    let found = cast(found[0].column(0), &DataType::Int64).unwrap();
    assert_eq!(
        found.as_primitive::<arrow::datatypes::Int64Type>().value(0),
        1
    );
    let current = "SELECT count(*) FROM dim WHERE _is_current";
    let current = session.query_arrow(current, &[]).unwrap();
    let current = cast(current[0].column(0), &DataType::Int64).unwrap();
    assert_eq!(
        current
            .as_primitive::<arrow::datatypes::Int64Type>()
            .value(0),
        1
    );
}

#[test]
fn mg_req_0664_delete_modes() {
    for deletes in [Deletes::Ignore, Deletes::Close, Deletes::Tombstone] {
        let mut session = session();
        let run = |at| history(segment_city(), deletes, HistoryInput::Other, at);
        input(session.as_mut(), "(1, NULL, 'a', 'x'), (2, NULL, 'b', 'y')");
        apply_history(session.as_mut(), &run(MARCH)).unwrap();
        input(session.as_mut(), "(1, NULL, 'a', 'x')");
        apply_history(session.as_mut(), &run(APRIL)).unwrap();
        let kept = version(1, Some("a"), ON_MARCH, END, CURRENT);
        let expected = match deletes {
            Deletes::Ignore => vec![kept, version(2, Some("b"), ON_MARCH, END, CURRENT)],
            Deletes::Close => vec![kept, version(2, Some("b"), ON_MARCH, ON_APRIL, CLOSED)],
            Deletes::Tombstone => vec![
                kept,
                version(2, Some("b"), ON_MARCH, ON_APRIL, CLOSED),
                version(2, None, ON_APRIL, END, TOMBSTONE),
            ],
        };
        assert_eq!(versions(session.as_mut()), expected, "{deletes:?}");
        // A key that comes back opens a new version, not deleted.
        input(session.as_mut(), "(1, NULL, 'a', 'x'), (2, NULL, 'b', 'y')");
        let may = APRIL + 30 * 86_400;
        apply_history(session.as_mut(), &run(may)).unwrap();
        let after = versions(session.as_mut());
        let last = after.last().unwrap();
        if deletes == Deletes::Ignore {
            assert_eq!(after.len(), 2);
        } else {
            assert_eq!(last.0, 2);
            assert_eq!((last.4, last.5), CURRENT);
            assert_eq!(last.2, "2026-05-01 00:00:00+00");
            let closed = after.iter().filter(|row| row.0 == 2 && !row.4).count();
            assert_eq!(closed, after.iter().filter(|row| row.0 == 2).count() - 1);
        }
    }
}

#[test]
fn mg_req_0665_history_rebuild_needs_flag() {
    let mut session = session();
    input(session.as_mut(), "(1, NULL, 'a', 'x')");
    let run = history(segment_city(), Deletes::Ignore, HistoryInput::Other, MARCH);
    apply_history(session.as_mut(), &run).unwrap();
    let refused = rebuild_history(session.as_mut(), "dim", RebuildHistory::Refused);
    let error = refused.unwrap_err();
    assert!(matches!(&error, EngineError::HistoryRebuild { table } if table == "dim"));
    assert_eq!(error.code(), Some("MG-0607"));
    assert_eq!(versions(session.as_mut()).len(), 1);
    rebuild_history(session.as_mut(), "dim", RebuildHistory::Confirmed).unwrap();
    assert!(session.query_arrow("SELECT * FROM dim", &[]).is_err());
}

#[test]
fn mg_req_0666_as_of_returns_valid_versions() {
    let mut session = session();
    let run = |at| history(segment_city(), Deletes::Tombstone, HistoryInput::Other, at);
    input(session.as_mut(), "(1, NULL, 'a', 'x'), (2, NULL, 'b', 'y')");
    apply_history(session.as_mut(), &run(MARCH)).unwrap();
    input(session.as_mut(), "(1, NULL, 'c', 'x')");
    apply_history(session.as_mut(), &run(APRIL)).unwrap();
    session.execute(AS_OF, &[]).unwrap();
    let segments = |session: &mut dyn DatabaseConnection, at: i128| {
        let at = SqlValue::TimestampTz(i64::try_from(at).unwrap() * 1_000_000);
        let sql = "SELECT id, segment FROM as_of('dim', CAST(? AS TIMESTAMPTZ)) ORDER BY id";
        let batches = session.query_arrow(sql, &[at]).unwrap();
        let mut rows = Vec::new();
        for batch in batches {
            let ids = batch
                .column(0)
                .as_primitive::<arrow::datatypes::Int64Type>();
            let names = batch.column(1).as_string::<i32>();
            for row in 0..batch.num_rows() {
                rows.push((ids.value(row), names.value(row).to_owned()));
            }
        }
        rows
    };
    let before_april = segments(session.as_mut(), APRIL - 1);
    assert_eq!(before_april, vec![(1, "a".to_owned()), (2, "b".to_owned())]);
    // At the change instant, the new version of 1 and no row for the tombstoned 2.
    assert_eq!(segments(session.as_mut(), APRIL), vec![(1, "c".to_owned())]);
    assert!(segments(session.as_mut(), MARCH - 1).is_empty());
}

#[test]
fn mg_req_0662_detection_time_keeps_microseconds() {
    let mut session = session();
    let at = |seconds: i128| Timestamp::from_unix_nanos(seconds * 1_000_000_000 + 123_456_789);
    for (values, seconds) in [
        ("(1, NULL, 'a', 'x')", MARCH),
        ("(1, NULL, 'b', 'x')", APRIL),
    ] {
        input(session.as_mut(), values);
        let mut run = history(segment_city(), Deletes::Ignore, HistoryInput::Other, 0);
        run.detected_at = at(seconds);
        apply_history(session.as_mut(), &run).unwrap();
    }
    let (march, april) = (
        "2026-03-01 00:00:00.123456+00",
        "2026-04-01 00:00:00.123456+00",
    );
    assert_eq!(
        versions(session.as_mut()),
        vec![
            version(1, Some("a"), march, april, CLOSED),
            version(1, Some("b"), april, END, CURRENT),
        ]
    );
}
