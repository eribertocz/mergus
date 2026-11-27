//! Tests of the sandboxed session of macro files of the real `Database` (MG-REQ-0641,
//! MG-REQ-0642).

use super::RealDatabase;
use crate::{CatalogMacro, Database, MacroRun};

/// Runs `script` in the sandboxed session of a real database.
fn run(script: &str) -> MacroRun {
    RealDatabase::new(&std::env::temp_dir().join("mergus-db-sandbox"))
        .run_macros(script)
        .unwrap()
}

#[test]
fn mg_req_0641_macros_read_from_the_catalog() {
    let run = run(
        "CREATE MACRO safe_div(a, b) AS CASE WHEN b = 0 THEN NULL ELSE a / b END;\n\
         CREATE MACRO clean_sales(t) AS TABLE SELECT * FROM query_table(t) WHERE status <> 'VOID';\n",
    );
    assert_eq!(run.refused, None);
    assert_eq!(run.changed, Vec::<String>::new());
    let names: Vec<(&str, bool)> = run
        .macros
        .iter()
        .map(|found| (found.name.as_str(), found.table))
        .collect();
    assert_eq!(names, [("clean_sales", true), ("safe_div", false)]);
    let CatalogMacro { definition, .. } = &run.macros[1];
    assert!(definition.contains("CASE"), "{definition}");
    assert!(run.builtins.iter().any(|name| name == "sum"));
    assert!(!run.builtins.iter().any(|name| name == "safe_div"));
}

#[test]
fn mg_req_0641_other_objects_are_changes() {
    let run = run("CREATE MACRO one() AS 1; CREATE TABLE t (a INTEGER); CREATE SCHEMA s;");
    assert_eq!(run.refused, None);
    assert!(
        run.changed
            .iter()
            .any(|row| row.starts_with("duckdb_tables: ")),
        "{:?}",
        run.changed
    );
    assert!(
        run.changed
            .iter()
            .any(|row| row.starts_with("duckdb_schemas: ")),
        "{:?}",
        run.changed
    );
}

#[test]
fn mg_req_0641_sandbox_refuses_files_network_and_settings() {
    for script in [
        "SET threads = 1;",
        "ATTACH 'other.duckdb';",
        "INSTALL httpfs;",
        "LOAD httpfs;",
        "COPY (SELECT 1) TO 'mg_req_0641_copy.csv';",
        "CREATE MACRO m() AS TABLE SELECT * FROM read_csv('mg_req_0641.csv');\
         SELECT * FROM m();",
        "CREATE MACRO m() AS {{ body }};",
    ] {
        let run = run(script);
        assert!(run.refused.is_some(), "`{script}` was not refused");
    }
    assert!(!std::path::Path::new("mg_req_0641_copy.csv").exists());
}

#[test]
fn mg_req_0644_macro_definitions_parse_back() {
    use crate::SqlParser;
    use crate::real::RealSqlParser;
    let run = run(
        "CREATE MACRO safe_div(a, b) AS CASE WHEN b = 0 THEN NULL ELSE a / b END;\n\
         CREATE MACRO margin(p, c) AS safe_div(p - c, p);\n\
         CREATE MACRO clean_sales(t) AS TABLE SELECT * FROM query_table(t) WHERE status <> 'VOID';\n",
    );
    let parser = RealSqlParser::new();
    for found in &run.macros {
        let body = if found.table {
            found.definition.clone()
        } else {
            ["SELECT ", found.definition.as_str()].concat()
        };
        let tree = parser.parse(&body).unwrap();
        assert!(tree.starts_with("{\"error\":false"), "{body}: {tree}");
        if found.name == "margin" {
            assert!(tree.contains("\"function_name\":\"safe_div\""), "{tree}");
        }
    }
}
