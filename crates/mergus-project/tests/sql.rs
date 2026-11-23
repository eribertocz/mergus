//! Tests of the checks of `.sql` models (MG-REQ-0193, MG-REQ-0307, MG-REQ-0308, MG-REQ-0373,
//! MG-REQ-0374). The trees of `sql/` were recorded from `json_serialize_sql` of the `duckdb`
//! crate 1.20001.0 (features `bundled` and `json`); the real parser arrives with #123.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{
    ConfigFile, Configuration, KnownTables, Layout, LayoutError, Model, ModelLanguage,
    ProjectError, Publication, Source, check_sql, check_sql_models, read_sql_references,
};
use mergus_sim::{SimFileSystem, SimSqlParser};
use serde_json::{Value, json};

/// The path every checked model has.
const PATH: &str = "models/marts/fact_sales.sql";

/// The text and the recorded tree of the fixture `name` of `sql/`.
macro_rules! fixture {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!("sql/", $name, ".sql")),
            include_str!(concat!("sql/", $name, ".json")),
        )
    };
}

/// Every fixture: its name, its SQL text and the tree the real parser gives for it.
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

/// A parser that knows the tree of every fixture.
fn parser() -> SimSqlParser {
    let parser = SimSqlParser::new();
    for (_, sql, tree) in FIXTURES {
        parser.record(*sql, *tree);
    }
    parser
}

/// The SQL text of the fixture `name`.
fn sql(name: &str) -> &'static str {
    let found = FIXTURES.iter().find(|(fixture, _, _)| *fixture == name);
    assert!(found.is_some(), "no fixture `{name}`");
    found.map_or("", |(_, sql, _)| *sql)
}

fn config(path: &str, value: Value) -> ConfigFile {
    ConfigFile {
        path: path.to_owned(),
        canonical: mergus_contracts::canonical_json(&value),
        value,
    }
}

fn model(name: &str, path: &str, language: ModelLanguage) -> Model {
    Model {
        name: name.to_owned(),
        path: path.to_owned(),
        language,
        publication: Publication::Intermediate,
    }
}

/// A project with the models `stg_orders` and `dim_customer` (Python), the source `erp` with
/// the table `orders`, and the seed `countries`.
fn project() -> (Layout, Configuration) {
    let layout = Layout {
        models: vec![
            model(
                "stg_orders",
                "models/staging/stg_orders.sql",
                ModelLanguage::Sql,
            ),
            model(
                "dim_customer",
                "models/marts/dim_customer.py",
                ModelLanguage::Python,
            ),
        ],
    };
    let configuration = Configuration {
        project: config(
            "project.toml",
            json!({"name": "shop", "seeds": {"countries": {}}}),
        ),
        sources: vec![Source {
            name: "erp".to_owned(),
            file: config("sources/erp.toml", json!({"tables": {"orders": {}}})),
        }],
    };
    (layout, configuration)
}

/// The problems of the fixture `name`, checked as the model at [`PATH`].
fn problems(name: &str) -> Vec<ProjectError> {
    let (layout, configuration) = project();
    let known = KnownTables::new(&layout, &configuration);
    let checked = check_sql(PATH, sql(name).as_bytes(), &known, &parser());
    assert!(checked.is_ok(), "{name}: {checked:?}");
    checked.unwrap_or_default()
}

/// The `(code, line, column)` of each problem of the fixture `name`.
fn found(name: &str) -> Vec<(&'static str, usize, usize)> {
    problems(name)
        .iter()
        .map(|problem| {
            (
                problem.code(),
                problem.line().unwrap_or(0),
                problem.column().unwrap_or(0),
            )
        })
        .collect()
}

/// Asserts that the fixture `name` has exactly one MG-REQ-0374 error, at `line` and `column`,
/// whose message names `construct`.
fn assert_rejected(name: &str, line: usize, column: usize, construct: &str) {
    let problems = problems(name);
    assert_eq!(problems.len(), 1, "{name}: {problems:?}");
    let problem = &problems[0];
    assert_eq!(
        (
            problem.code(),
            problem.path(),
            problem.line(),
            problem.column()
        ),
        ("MG-0314", PATH, Some(line), Some(column)),
        "{name}: {problem:?}"
    );
    assert!(problem.to_string().contains(construct), "{name}: {problem}");
}

#[test]
fn mg_req_0307_unknown_reference_rejected() {
    let problems = problems("misspelled");
    assert_eq!(
        problems,
        [ProjectError::UnknownReference {
            path: PATH.to_owned(),
            line: 2,
            column: 8,
            name: "stg_ordr".to_owned(),
            suggestion: Some("stg_orders".to_owned()),
        }]
    );
    assert_eq!(problems[0].code(), "MG-0313");
    let message = problems[0].to_string();
    assert!(message.contains("did you mean `stg_orders`?"), "{message}");
    assert!(message.contains("line 2 column 8"), "{message}");
}

#[test]
fn mg_req_0373_references_resolved() {
    // MG-REQ-0373.1: a model of the project, compared without case.
    assert_eq!(found("model"), []);
    assert_eq!(found("model_any_case"), []);
    // MG-REQ-0373.2: a source table or a seed in schema `raw`, compared without case.
    assert_eq!(found("source_table"), []);
    assert_eq!(found("seed"), []);
    // Any other reference is the error of MG-REQ-0307, at the reference.
    assert_eq!(found("model_in_raw"), [("MG-0313", 1, 15)]);
    assert_eq!(found("source_without_schema"), [("MG-0313", 1, 15)]);
    assert_eq!(found("other_schema"), [("MG-0313", 1, 15)]);
    assert_eq!(found("with_catalog"), [("MG-0313", 1, 15)]);
    // A visible CTE is not a reference, even when it shadows a model; a CTE is visible in its
    // query, in the CTEs after it and, when recursive, in itself; nowhere else.
    assert_eq!(found("shadowing_cte"), []);
    assert_eq!(found("recursive_cte"), []);
    assert_eq!(found("cte_out_of_scope"), [("MG-0313", 2, 72)]);
    assert_eq!(found("cte_used_before_defined"), [("MG-0313", 1, 34)]);
}

#[test]
fn mg_req_0374_side_effects_rejected() {
    // MG-REQ-0374.1: a parser error, or other than exactly one statement.
    assert_rejected("syntax_error", 2, 17, "syntax error at or near \"WHERE\"");
    assert_rejected("two_statements", 2, 8, "2 statements");
    assert_rejected("empty", 1, 1, "no statement");
    for name in [
        "attach", "copy", "insert", "create", "install", "load", "set", "pragma",
    ] {
        assert_rejected(name, 1, 1, "Only SELECT statements");
    }
    // MG-REQ-0374.2: table functions other than `range`, `generate_series` and `unnest`.
    assert_rejected("read_csv", 2, 6, "table function `read_csv`");
    assert_rejected("read_parquet", 1, 15, "table function `read_parquet`");
    assert_rejected("glob", 1, 15, "table function `glob`");
    assert_rejected("query", 1, 15, "table function `query`");
    assert_eq!(found("allowed_table_functions"), []);
    // MG-REQ-0374.3: a base-table name that is not a name of MG-REQ-0305.
    assert_rejected("file_name", 1, 15, "table name `orders.csv`");
    assert_rejected(
        "url",
        1,
        15,
        "table name `https://example.com/orders.parquet`",
    );
    // MG-REQ-0374.4: `getenv`, `nextval`, `currval` and `setseed`, wherever they are called.
    assert_rejected("getenv", 1, 8, "function `getenv`");
    assert_rejected("nextval", 1, 8, "function `nextval`");
    assert_rejected("currval", 1, 8, "function `currval`");
    assert_rejected("setseed", 1, 8, "function `setseed`");
    assert_rejected("nested_function", 1, 26, "function `getenv`");
    assert_rejected("function_in_subquery", 1, 46, "function `nextval`");
}

#[test]
fn mg_req_0374_text_that_is_not_utf8_rejected() {
    let (layout, configuration) = project();
    let known = KnownTables::new(&layout, &configuration);
    let problems = check_sql(PATH, b"SELECT \xff", &known, &parser()).unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].code(), "MG-0314");
}

/// Writes the `.sql` models of `models`, by name and fixture, into a simulated project, and
/// checks them.
fn check_project(models: &[(&str, &str)]) -> Result<(), LayoutError> {
    let fs = SimFileSystem::new();
    let (mut layout, configuration) = project();
    for (name, fixture) in models {
        let path = format!("models/staging/{name}.sql");
        let full = Path::new("/project").join(&path);
        if let Some(parent) = full.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&full, sql(fixture).as_bytes())?;
        layout.models.push(model(name, &path, ModelLanguage::Sql));
    }
    layout.models.remove(0);
    check_sql_models(
        &fs,
        Path::new("/project"),
        &layout,
        &configuration,
        &parser(),
    )
}

#[test]
fn mg_req_0308_side_effect_sql_rejected() {
    assert_eq!(check_project(&[("stg_orders", "source_table")]), Ok(()));
    let side_effects = [
        ("a_insert", "insert"),
        ("b_attach", "attach"),
        ("c_install", "install"),
        ("d_set", "set"),
        ("e_read_parquet", "read_parquet"),
        ("f_url", "url"),
        ("g_getenv", "getenv"),
    ];
    let Err(LayoutError::Invalid(problems)) = check_project(&side_effects) else {
        panic!("side effects are rejected");
    };
    let rejected: Vec<(&str, &str)> = problems
        .iter()
        .map(|problem| (problem.code(), problem.path()))
        .collect();
    let expected: Vec<(&str, String)> = side_effects
        .iter()
        .map(|(name, _)| ("MG-0314", format!("models/staging/{name}.sql")))
        .collect();
    let expected: Vec<(&str, &str)> = expected
        .iter()
        .map(|(code, path)| (*code, path.as_str()))
        .collect();
    assert_eq!(rejected, expected);
}

#[test]
fn python_models_are_not_parsed() {
    let fs = SimFileSystem::new();
    let (layout, configuration) = project();
    let only_python = Layout {
        models: layout.models[1..].to_vec(),
    };
    let parser = SimSqlParser::new();
    assert_eq!(
        check_sql_models(
            &fs,
            Path::new("/project"),
            &only_python,
            &configuration,
            &parser
        ),
        Ok(())
    );
}

/// The resolved references of each `.sql` model of `models`, by name and fixture.
fn references(models: &[(&str, &str)]) -> Result<Vec<(String, Vec<String>)>, LayoutError> {
    let fs = SimFileSystem::new();
    let (mut layout, configuration) = project();
    for (name, fixture) in models {
        let path = format!("models/staging/{name}.sql");
        let full = Path::new("/project").join(&path);
        if let Some(parent) = full.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&full, sql(fixture).as_bytes())?;
        layout.models.push(model(name, &path, ModelLanguage::Sql));
    }
    layout.models.remove(0);
    let found = read_sql_references(
        &fs,
        Path::new("/project"),
        &layout,
        &configuration,
        &parser(),
    )?;
    Ok(found.into_iter().collect())
}

#[test]
fn mg_req_0373_resolved_references_listed() {
    let found = references(&[
        ("stg_orders", "source_table"),
        ("a_model", "model"),
        ("b_any_case", "model_any_case"),
        ("d_seed", "seed"),
        ("e_shadowing", "shadowing_cte"),
        ("f_recursive", "recursive_cte"),
    ]);
    let owned = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect();
    // Lowercase, models bare, raw tables with `raw.`, CTEs left out; no
    // Python model is listed.
    assert_eq!(
        found,
        Ok(vec![
            ("a_model".to_owned(), owned(&["stg_orders"])),
            ("b_any_case".to_owned(), owned(&["stg_orders"])),
            ("d_seed".to_owned(), owned(&["raw.seed__countries"])),
            ("e_shadowing".to_owned(), owned(&[])),
            ("f_recursive".to_owned(), owned(&[])),
            ("stg_orders".to_owned(), owned(&["raw.erp__orders"])),
        ])
    );
    let Err(LayoutError::Invalid(problems)) = references(&[("a_bad", "misspelled")]) else {
        panic!("a model with a problem has no references");
    };
    assert_eq!(problems[0].code(), "MG-0313");
}
