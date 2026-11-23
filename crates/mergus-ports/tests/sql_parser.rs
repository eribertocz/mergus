//! Tests of the real `SqlParser` (MG-REQ-0193): it gives the trees recorded in
//! `crates/mergus-project/tests/sql/`, which the simulated parser replays, and executes nothing.

use std::path::Path;
use std::sync::Arc;

use mergus_ports::real::{RealFileSystem, RealSqlParser};
use mergus_ports::{FileSystem, SqlParser};

/// The name, the SQL text and the recorded tree of the fixture `name`.
macro_rules! fixture {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!("../../mergus-project/tests/sql/", $name, ".sql")),
            include_str!(concat!("../../mergus-project/tests/sql/", $name, ".json")),
        )
    };
}

/// Every recorded fixture.
const FIXTURES: &[(&str, &str, &str)] = &[
    fixture!("allowed_table_functions"),
    fixture!("attach"),
    fixture!("copy"),
    fixture!("create"),
    fixture!("cte_out_of_scope"),
    fixture!("cte_used_before_defined"),
    fixture!("currval"),
    fixture!("empty"),
    fixture!("file_name"),
    fixture!("function_in_subquery"),
    fixture!("getenv"),
    fixture!("glob"),
    fixture!("insert"),
    fixture!("install"),
    fixture!("load"),
    fixture!("misspelled"),
    fixture!("model"),
    fixture!("model_any_case"),
    fixture!("model_in_raw"),
    fixture!("nested_function"),
    fixture!("nextval"),
    fixture!("other_schema"),
    fixture!("pragma"),
    fixture!("query"),
    fixture!("read_csv"),
    fixture!("read_parquet"),
    fixture!("recursive_cte"),
    fixture!("seed"),
    fixture!("set"),
    fixture!("setseed"),
    fixture!("shadowing_cte"),
    fixture!("source_table"),
    fixture!("source_without_schema"),
    fixture!("syntax_error"),
    fixture!("two_statements"),
    fixture!("url"),
    fixture!("with_catalog"),
];

#[test]
fn mg_req_0193_sql_parsed_without_execution() {
    let parser = RealSqlParser::new();
    for (name, sql, tree) in FIXTURES {
        assert_eq!(parser.parse(sql).unwrap(), *tree, "fixture `{name}`");
    }

    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("mg_req_0193_copy.csv");
    let files = RealFileSystem;
    if files.exists(&out).unwrap() {
        files.remove_file(&out).unwrap();
    }
    let copy = format!("COPY (SELECT 1) TO '{}'", out.display());
    let tree = parser.parse(&copy).unwrap();
    assert!(tree.contains("Only SELECT statements"), "{tree}");
    assert!(!files.exists(&out).unwrap(), "the text was executed");
}

#[test]
fn real_sql_parser_is_shared_between_threads() {
    let parser = Arc::new(RealSqlParser::new());
    let (_, sql, tree) = FIXTURES[0];
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let parser = Arc::clone(&parser);
            std::thread::spawn(move || parser.parse(sql).unwrap())
        })
        .collect();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), tree);
    }
}
