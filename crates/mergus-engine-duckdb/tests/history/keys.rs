//! Keys a history run cannot apply: repeated (MG-REQ-0663.7) or NULL (MG-REQ-0663.8).

use mergus_engine_duckdb::{Deletes, EngineError, HistoryInput, apply_history};

use super::{
    APRIL, CURRENT, END, MARCH, ON_MARCH, history, input, segment_city, session, version, versions,
};

#[test]
fn mg_req_0662_repeated_key_fails_and_changes_nothing() {
    let mut session = session();
    input(session.as_mut(), "(1, NULL, 'a', 'x')");
    let run = |at| history(segment_city(), Deletes::Close, HistoryInput::Other, at);
    apply_history(session.as_mut(), &run(MARCH)).unwrap();
    input(session.as_mut(), "(1, NULL, 'b', 'x'), (1, NULL, 'c', 'x')");
    let error = apply_history(session.as_mut(), &run(APRIL)).unwrap_err();
    assert!(matches!(&error, EngineError::HistoryRepeatedKey { key, .. } if key == "1"));
    assert_eq!(error.code(), Some("MG-0608"));
    let unchanged = vec![version(1, Some("a"), ON_MARCH, END, CURRENT)];
    assert_eq!(versions(session.as_mut()), unchanged);
}

#[test]
fn mg_req_0663_null_key_fails_and_writes_nothing() {
    let mut session = session();
    input(
        session.as_mut(),
        "(1, NULL, 'a', 'x'), (NULL, NULL, 'b', 'y')",
    );
    let run = history(segment_city(), Deletes::Close, HistoryInput::Other, MARCH);
    let error = apply_history(session.as_mut(), &run).unwrap_err();
    assert!(matches!(&error, EngineError::HistoryNullKey { key, .. } if key == "id"));
    assert_eq!(error.code(), Some("MG-0610"));
    assert!(session.query_arrow("SELECT * FROM dim", &[]).is_err());
}
