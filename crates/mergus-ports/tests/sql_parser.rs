//! MG-REQ-0193: the real `SqlParser` returns DuckDB's `json_serialize_sql` tree and executes
//! nothing.

use std::path::Path;

use mergus_ports::real::{RealFileSystem, RealSqlParser};
use mergus_ports::{FileSystem, SqlParser};

/// The tree of `SELECT 1`, as `json_serialize_sql` gives it.
const SELECT_1: &str = concat!(
    r#"{"error":false,"statements":[{"node":{"type":"SELECT_NODE","modifiers":[],"#,
    r#""cte_map":{"map":[]},"select_list":[{"class":"CONSTANT","type":"VALUE_CONSTANT","#,
    r#""alias":"","query_location":7,"query_location_length":1,"literal":{"kind":"INTEGER","#,
    r#""text":"1"}}],"from_table":{"type":"EMPTY","alias":"","sample":null,"#,
    r#""query_location":18446744073709551615,"query_location_length":0},"where_clause":null,"#,
    r#""group_expressions":[],"group_sets":[],"aggregate_handling":"STANDARD_HANDLING","#,
    r#""having":null,"sample":null,"qualify":null},"named_param_map":[]}]}"#,
);

#[test]
fn mg_req_0193_sql_parsed_without_execution() {
    let parser = RealSqlParser::new();
    assert_eq!(parser.parse("SELECT 1").unwrap(), SELECT_1);
    // Executing these would fail: the table, the sequence and the file do not exist, and the
    // database may not read files. Parsing them succeeds.
    for sql in [
        "SELECT * FROM missing_table",
        "SELECT nextval('missing_sequence')",
        "SELECT * FROM read_csv('/missing/orders.csv')",
    ] {
        let tree = parser.parse(sql).unwrap();
        assert!(
            tree.starts_with(r#"{"error":false,"statements":[{"#),
            "{sql}: {tree}"
        );
    }
    // A text that is not one SELECT is not run: its tree reports the error.
    let tree = parser.parse("CREATE TABLE created AS SELECT 1").unwrap();
    assert!(tree.starts_with(r#"{"error":true"#), "{tree}");
    let tree = parser.parse("SELECT * FROM created").unwrap();
    assert!(tree.contains(r#""table_name":"created""#), "{tree}");
    let tree = parser.parse("SELEC 1").unwrap();
    assert!(tree.contains(r#""error_type":"parser""#), "{tree}");
}

/// The trees `mergus-project` tests with were recorded from this parser: each
/// `crates/mergus-project/tests/sql/<case>.json` is the tree of `<case>.sql`.
#[test]
fn recorded_trees_are_those_of_the_real_parser() {
    let parser = RealSqlParser::new();
    let files = RealFileSystem;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mergus-project/tests/sql");
    let mut checked = 0;
    for path in files.list_dir(&dir).unwrap() {
        if path.extension().is_some_and(|extension| extension == "sql") {
            let sql = String::from_utf8(files.read(&path).unwrap()).unwrap();
            let tree = String::from_utf8(files.read(&path.with_extension("json")).unwrap());
            assert_eq!(
                parser.parse(&sql).unwrap(),
                tree.unwrap(),
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no recorded tree in {}", dir.display());
}
