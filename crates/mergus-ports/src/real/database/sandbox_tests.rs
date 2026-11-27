//! Tests of the sandboxed session of macro files of the real `Database` (MG-REQ-0641,
//! MG-REQ-0642).

use std::path::Path;

use super::RealDatabase;
use crate::real::RealFileSystem;
use crate::{CatalogMacro, Database, FileSystem, MacroRun};

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

/// The refusal of `script` in the sandboxed session; fails when it is not refused.
fn refusal(script: &str) -> String {
    let run = run(script);
    assert_eq!(run.macros, Vec::<CatalogMacro>::new(), "`{script}`");
    run.refused
        .unwrap_or_else(|| panic!("`{script}` was not refused"))
}

#[test]
fn mg_req_0641_sandbox_refuses_files_network_and_settings() {
    // The paths are relative to the crate directory, where `cargo test` runs.
    let files = RealFileSystem;
    let csv = Path::new("mg_req_0641_rates.csv");
    let attached = Path::new("mg_req_0641_other.duckdb");
    let copy = Path::new("mg_req_0641_copy.csv");
    files.write(csv, b"day,rate\n2026-01-01,1.5\n").unwrap();
    for path in [attached, copy] {
        if files.exists(path).unwrap() {
            files.remove_file(path).unwrap();
        }
    }
    let disabled = "file system operations are disabled by configuration";
    for script in [
        "CREATE MACRO m() AS TABLE SELECT * FROM read_csv('mg_req_0641_rates.csv');",
        "ATTACH 'mg_req_0641_other.duckdb';",
        "COPY (SELECT 1) TO 'mg_req_0641_copy.csv';",
    ] {
        let refused = refusal(script);
        assert!(refused.contains(disabled), "`{script}`: {refused}");
    }
    files.remove_file(csv).unwrap();
    assert!(
        !files.exists(attached).unwrap(),
        "the ATTACH target was created"
    );
    assert!(!files.exists(copy).unwrap(), "the COPY target was created");
    let locked = refusal("SET threads = 1;");
    assert!(
        locked.contains("the configuration has been locked"),
        "{locked}"
    );
    for script in [
        "INSTALL httpfs;",
        "LOAD httpfs;",
        "CREATE MACRO m() AS {{ body }};",
    ] {
        refusal(script);
    }
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
        let tree = if found.table {
            parser.parse(&found.definition).unwrap()
        } else {
            parser.parse_select_list(&found.definition).unwrap()
        };
        let body = &found.definition;
        assert!(tree.starts_with("{\"error\":false"), "{body}: {tree}");
        if found.name == "margin" {
            assert!(tree.contains("\"function_name\":\"safe_div\""), "{tree}");
        }
    }
}
