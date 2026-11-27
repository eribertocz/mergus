//! MG-REQ-0667: a history model reads the changes view of a raw table.

use mergus_engine_duckdb::{ChangeDetection, Deletes, HistoryInput, apply_history};
use mergus_ports::DatabaseConnection;

use super::{APRIL, CLOSED, CURRENT, END, MARCH, TOMBSTONE, history, session, version, versions};

/// Replaces the changes view `src` with `rows`, a `VALUES` list of
/// `(id, segment, _load_id, _extracted_at, _deleted, _deleted_at)`.
#[cfg(test)]
fn changes(session: &mut dyn DatabaseConnection, values: &str) {
    session.execute("DROP TABLE IF EXISTS src", &[]).unwrap();
    let create = "CREATE TABLE src (id BIGINT, segment VARCHAR, _load_id VARCHAR, \
                  _extracted_at TIMESTAMPTZ, _deleted BOOLEAN, _deleted_at TIMESTAMPTZ)";
    session.execute(create, &[]).unwrap();
    let mut insert = "INSERT INTO src VALUES ".to_owned();
    insert.push_str(values);
    session.execute(&insert, &[]).unwrap();
}

#[test]
fn mg_req_0667_history_reads_changes_view() {
    let mut session = session();
    let by = ChangeDetection::Differs(vec!["segment".to_owned()]);
    let read = |ok: &[&str], after: Option<&str>| HistoryInput::Changes {
        ok_loads: ok.iter().map(|load| (*load).to_owned()).collect(),
        after: after.map(str::to_owned),
    };
    // Two loads between runs keep the intermediate version of 1; load L3 is not `ok`.
    changes(
        session.as_mut(),
        "(1, 'b', 'L2', '2026-01-20 00:00:00+00', false, NULL), \
         (1, 'a', 'L1', '2026-01-10 00:00:00+00', false, NULL), \
         (2, 'x', 'L1', '2026-01-10 00:00:00+00', false, NULL), \
         (2, 'x', 'L2', '2026-01-20 00:00:00+00', true, '2026-01-21 00:00:00+00'), \
         (1, 'z', 'L3', '2026-01-25 00:00:00+00', false, NULL)",
    );
    let first = history(
        by.clone(),
        Deletes::Tombstone,
        read(&["L1", "L2"], None),
        MARCH,
    );
    let applied = apply_history(session.as_mut(), &first).unwrap();
    assert_eq!(applied, Some("L2".to_owned()));
    let (jan10, jan20, jan21) = (
        "2026-01-10 00:00:00+00",
        "2026-01-20 00:00:00+00",
        "2026-01-21 00:00:00+00",
    );
    assert_eq!(
        versions(session.as_mut()),
        vec![
            version(1, Some("a"), jan10, jan20, CLOSED),
            version(1, Some("b"), jan20, END, CURRENT),
            version(2, Some("x"), jan10, jan21, CLOSED),
            version(2, None, jan21, END, TOMBSTONE),
        ]
    );
    // A later run reads only loads past the last applied one.
    let second = history(
        by,
        Deletes::Tombstone,
        read(&["L1", "L2", "L4"], Some("L2")),
        APRIL,
    );
    changes(
        session.as_mut(),
        "(1, 'a', 'L1', '2026-01-10 00:00:00+00', false, NULL), \
         (1, 'c', 'L4', '2026-02-01 00:00:00+00', false, NULL)",
    );
    let applied = apply_history(session.as_mut(), &second).unwrap();
    assert_eq!(applied, Some("L4".to_owned()));
    let rows = versions(session.as_mut());
    assert_eq!(rows.len(), 5);
    assert_eq!(
        rows[2],
        version(1, Some("c"), "2026-02-01 00:00:00+00", END, CURRENT)
    );
    // The raw metadata columns are not columns of the history table.
    let described = session.describe("SELECT * FROM dim", &[]).unwrap();
    assert!(
        described
            .iter()
            .all(|column| !column.name.starts_with("_load"))
    );
    assert!(!described.iter().any(|column| column.name == "_deleted"));
    // Nothing new: the last applied load stays.
    let applied = apply_history(session.as_mut(), &second).unwrap();
    assert_eq!(applied, Some("L4".to_owned()));
    assert_eq!(versions(session.as_mut()).len(), 5);
}
