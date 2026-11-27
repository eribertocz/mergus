//! Macros: DuckDB macros only, unique safe names, fingerprint by use.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mergus_ports::{CatalogMacro, MacroRun};
use mergus_project::{LayoutError, Macros, ProjectError, read_macros, sql_calls};
use mergus_sim::{DatabaseFaults, SimDatabase, SimSqlParser};

use super::{ROOT, project};

/// A scalar macro named `name` defined as `definition`.
fn scalar(name: &str, definition: &str) -> CatalogMacro {
    CatalogMacro {
        name: name.to_owned(),
        table: false,
        definition: definition.to_owned(),
    }
}

/// A run that created `macros` and nothing else, with the built-in `sum`.
fn created(macros: Vec<CatalogMacro>) -> MacroRun {
    MacroRun {
        builtins: vec!["sum".to_owned()],
        macros,
        ..MacroRun::default()
    }
}

/// A tree of one statement that calls each function of `calls`.
fn tree(calls: &[&str]) -> String {
    let functions: Vec<String> = calls
        .iter()
        .map(|name| format!(r#"{{"class":"FUNCTION","function_name":"{name}","children":[]}}"#))
        .collect();
    format!(
        r#"{{"error":false,"statements":[{{"node":{{"type":"SELECT_NODE","select_list":[{}]}}}}]}}"#,
        functions.join(",")
    )
}

/// Reads the macros of a project holding `files`, each run as `runs` records it.
fn read(
    files: &[(&str, &str)],
    runs: &[(&str, MacroRun)],
    parser: &SimSqlParser,
) -> Result<Macros, LayoutError> {
    let bytes: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(path, text)| (*path, text.as_bytes()))
        .collect();
    let fs = project(&bytes)?;
    let database = SimDatabase::new(1, DatabaseFaults::default());
    for (script, run) in runs {
        database.record_macro_run(*script, run.clone());
    }
    read_macros(&fs, Path::new(ROOT), &database, parser)
}

/// The problems of a read that has some.
macro_rules! problems {
    ($read:expr) => {
        match $read {
            Err(LayoutError::Invalid(problems)) => problems,
            other => panic!("expected problems, got {other:?}"),
        }
    };
}

/// A macro file of only one scalar macro.
const GOOD: &str = "CREATE MACRO safe_div(a, b) AS CASE WHEN b = 0 THEN NULL ELSE a / b END;\n";
/// A macro file that also creates a table.
const TABLE: &str = "CREATE MACRO t() AS TABLE SELECT 1;\nCREATE TABLE t (a INTEGER);\n";
/// A macro file with text templating.
const TEMPLATED: &str = "CREATE MACRO m() AS {{ body }};\n";
/// A macro file that changes a setting.
const SET: &str = "SET threads = 1;\n";

/// The runs the real sandboxed session gives for the files of MG-REQ-0641.
fn sandbox_runs() -> [(&'static str, MacroRun); 4] {
    let refusal = |message: &str| MacroRun {
        refused: Some(message.to_owned()),
        ..MacroRun::default()
    };
    let table = MacroRun {
        changed: vec!["duckdb_tables: {'table_name': t}".to_owned()],
        ..MacroRun::default()
    };
    let definition = "CASE WHEN b = 0 THEN NULL ELSE a / b END";
    [
        (GOOD, created(vec![scalar("safe_div", definition)])),
        (TABLE, table),
        (
            TEMPLATED,
            refusal("Parser Error: syntax error at or near \"{\"\n\nLINE 1: CREATE MACRO"),
        ),
        (
            SET,
            refusal("Invalid Input Error: Cannot change configuration option \"threads\""),
        ),
    ]
}

#[test]
fn mg_req_0641_macros_only_create_macro() {
    let (good, table, templated, set) = (GOOD, TABLE, TEMPLATED, SET);
    let parser = SimSqlParser::new();
    parser.record("SELECT CASE WHEN b = 0 THEN NULL ELSE a / b END", tree(&[]));
    let runs = sandbox_runs();
    let only_good = read(&[("macros/finance.sql", good)], &runs, &parser).unwrap();
    let names: Vec<&String> = only_good.macros.keys().collect();
    assert_eq!(names, ["safe_div"]);
    assert_eq!(only_good.macros["safe_div"].path, "macros/finance.sql");
    let files = [
        ("macros/a_table.sql", table),
        ("macros/b_templated.sql", templated),
        ("macros/c_set.sql", set),
        ("macros/finance.sql", good),
        ("macros/notes.txt", "not a macro file"),
    ];
    let found = problems!(read(&files, &runs, &parser));
    let shown: Vec<(&str, &str, String)> = found
        .iter()
        .map(|problem| (problem.code(), problem.path(), problem.to_string()))
        .collect();
    assert_eq!(shown.len(), 3, "{shown:#?}");
    assert!(shown.iter().all(|(code, _, _)| *code == "MG-0604"));
    assert_eq!(shown[0].1, "macros/a_table.sql");
    assert!(shown[0].2.contains("`duckdb_tables()`"), "{}", shown[0].2);
    assert_eq!(shown[1].1, "macros/b_templated.sql");
    assert!(shown[1].2.contains("syntax error"), "{}", shown[1].2);
    assert!(!shown[1].2.contains("LINE 1"), "{}", shown[1].2);
    assert_eq!(shown[2].1, "macros/c_set.sql");
}

#[test]
fn mg_req_0642_macro_name_clash_rejected() {
    let first = "CREATE MACRO Safe_Div(a, b) AS a / b;\n";
    let second = "CREATE MACRO safe_div(a, b) AS a / b;\n";
    let builtin = "CREATE MACRO SUM(a) AS a;\n";
    let reserved = "CREATE MACRO As_Of(t) AS t;\n";
    let runs = [
        (first, created(vec![scalar("Safe_Div", "(a / b)")])),
        (second, created(vec![scalar("safe_div", "(a / b)")])),
        (builtin, created(vec![scalar("SUM", "a")])),
        (reserved, created(vec![scalar("As_Of", "t")])),
    ];
    let files = [
        ("macros/a.sql", first),
        ("macros/b.sql", second),
        ("macros/c.sql", builtin),
        ("macros/d.sql", reserved),
    ];
    let found = problems!(read(&files, &runs, &SimSqlParser::new()));
    assert_eq!(
        found,
        [
            ProjectError::DuplicateMacro {
                name: "safe_div".to_owned(),
                first: "macros/a.sql".to_owned(),
                path: "macros/b.sql".to_owned(),
            },
            ProjectError::MacroShadowsBuiltin {
                name: "sum".to_owned(),
                path: "macros/c.sql".to_owned(),
                builtin: "sum".to_owned(),
            },
            ProjectError::MacroShadowsBuiltin {
                name: "as_of".to_owned(),
                path: "macros/d.sql".to_owned(),
                builtin: "as_of".to_owned(),
            },
        ]
    );
    assert!(found.iter().all(|problem| problem.code() == "MG-0605"));
    assert!(found[0].to_string().contains("`macros/a.sql`"));
    assert!(found[0].to_string().contains("`macros/b.sql`"));
}

/// The models of MG-REQ-0644: name, text, and the function names of its tree.
const MODELS: [(&str, &str, &[&str]); 3] = [
    (
        "fact_margin",
        "SELECT MARGIN(price, cost) FROM raw.erp__orders",
        &["MARGIN"],
    ),
    (
        "fact_other",
        "SELECT other(price) FROM raw.erp__orders",
        &["other"],
    ),
    (
        "fact_plain",
        "SELECT sum(price) FROM raw.erp__orders",
        &["sum"],
    ),
];

/// A parser that knows the models of [`MODELS`] and the macro bodies of MG-REQ-0644, with
/// `safe_div` defined as `division`.
fn reuse_parser(division: &str) -> SimSqlParser {
    let parser = SimSqlParser::new();
    parser.record(["SELECT ", division].concat(), tree(&[]));
    parser.record("SELECT safe_div((p - c), p)", tree(&["safe_div", "-"]));
    parser.record("SELECT * FROM query_table(t)", tree(&["query_table"]));
    parser.record("SELECT (a + 1)", tree(&["+"]));
    for (_, sql, calls) in MODELS {
        parser.record(sql, tree(calls));
    }
    parser
}

/// The macros of MG-REQ-0644, with `safe_div` defined as `division`.
fn reuse_macros(division: &str, parser: &SimSqlParser) -> Result<Macros, LayoutError> {
    let finance = [
        "CREATE MACRO safe_div(a, b) AS ",
        division,
        ";\nCREATE MACRO margin(p, c) AS safe_div(p - c, p);\n\
         CREATE MACRO clean(t) AS TABLE SELECT * FROM query_table(t);\n",
    ]
    .concat();
    let other = "CREATE MACRO other(a) AS a + 1;\n";
    let table = CatalogMacro {
        name: "clean".to_owned(),
        table: true,
        definition: "SELECT * FROM query_table(t)".to_owned(),
    };
    let created_finance = created(vec![
        table,
        scalar("margin", "safe_div((p - c), p)"),
        scalar("safe_div", division),
    ]);
    let runs = [
        (finance.as_str(), created_finance),
        (other, created(vec![scalar("other", "(a + 1)")])),
    ];
    let files = [
        ("macros/finance.sql", finance.as_str()),
        ("macros/other.sql", other),
    ];
    read(&files, &runs, parser)
}

/// The fingerprint inputs of each model of [`MODELS`], by name, with `safe_div` defined as
/// `division`.
fn inputs_with(
    division: &str,
) -> Result<BTreeMap<&'static str, BTreeMap<String, String>>, LayoutError> {
    let parser = reuse_parser(division);
    let macros = reuse_macros(division, &parser)?;
    assert_eq!(
        macros.macros["margin"].calls,
        BTreeSet::from(["safe_div".to_owned()])
    );
    MODELS
        .iter()
        .map(|(name, sql, _)| {
            let calls = sql_calls(&parser, sql)?;
            Ok((*name, macros.fingerprint_inputs(&calls)))
        })
        .collect()
}

#[test]
fn mg_req_0644_macro_change_refingerprints_callers_only() {
    let before = inputs_with("CASE WHEN (b = 0) THEN NULL ELSE (a / b) END").unwrap();
    let after = inputs_with("CASE WHEN (b = 0) THEN 0 ELSE (a / b) END").unwrap();
    let keys: Vec<&String> = before["fact_margin"].keys().collect();
    assert_eq!(keys, ["macro:margin", "macro:safe_div"]);
    assert!(before["fact_margin"]["macro:safe_div"].starts_with("sha256:"));
    assert_ne!(before["fact_margin"], after["fact_margin"]);
    assert_eq!(
        before["fact_margin"]["macro:margin"],
        after["fact_margin"]["macro:margin"]
    );
    assert_eq!(before["fact_other"], after["fact_other"]);
    let other: Vec<&String> = before["fact_other"].keys().collect();
    assert_eq!(other, ["macro:other"]);
    assert_eq!(before["fact_plain"], BTreeMap::new());
    assert_eq!(after["fact_plain"], BTreeMap::new());
}

#[test]
fn mg_req_0644_each_overload_parsed_in_its_form() {
    let file = "CREATE MACRO pick(a) AS helper(a), (t) AS TABLE SELECT * FROM query_table(t);\n\
                CREATE MACRO helper(a) AS a;\n";
    let table = CatalogMacro {
        name: "pick".to_owned(),
        table: true,
        definition: "SELECT * FROM query_table(t)".to_owned(),
    };
    let runs = [(
        file,
        created(vec![
            scalar("helper", "a"),
            scalar("pick", "helper(a)"),
            table,
        ]),
    )];
    let parser = SimSqlParser::new();
    parser.record("SELECT a", tree(&[]));
    parser.record("SELECT helper(a)", tree(&["helper"]));
    parser.record("SELECT * FROM query_table(t)", tree(&["query_table"]));
    let macros = read(&[("macros/pick.sql", file)], &runs, &parser).unwrap();
    let pick = &macros.macros["pick"];
    assert!(pick.table);
    assert_eq!(pick.overloads.len(), 2);
    assert_eq!(pick.calls, BTreeSet::from(["helper".to_owned()]));
}
