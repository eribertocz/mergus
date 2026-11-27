//! Tests of SQL models (MG-REQ-0601 to MG-REQ-0604). The trees of `../sql/` were recorded
//! from the real `SqlParser`, and the schemas from the real `Database` (see
//! `mg_req_0604_schema_without_data` in `mergus-ports`).

mod parameters;
mod schema;

use std::collections::BTreeMap;
use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{
    ConfigFile, Configuration, KnownTables, Layout, LayoutError, Model, ModelLanguage, Publication,
    Source, read_sql_references, read_sql_step,
};
use mergus_sim::{SimFileSystem, SimSqlParser};
use serde_json::{Value, json};

/// The path every model of these tests has, unless it says otherwise.
const PATH: &str = "models/marts/fact_sales.sql";

/// The text and the recorded tree of the fixture `name` of `sql/`.
macro_rules! fixture {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!("../sql/", $name, ".sql")),
            include_str!(concat!("../sql/", $name, ".json")),
        )
    };
}

/// The fixtures of these tests: name, SQL text, tree of the real parser.
const FIXTURES: &[(&str, &str, &str)] = &[
    fixture!("attach"),
    fixture!("copy"),
    fixture!("create"),
    fixture!("cte_shadows_model"),
    fixture!("from_first"),
    fixture!("insert"),
    fixture!("model"),
    fixture!("numbered"),
    fixture!("parameters"),
    fixture!("positional"),
    fixture!("pragma"),
    fixture!("set"),
    fixture!("shadowing_cte"),
    fixture!("source_table"),
    fixture!("two_statements"),
    fixture!("typed"),
    fixture!("unknown_parameter"),
    fixture!("with_select"),
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

fn model(name: &str, path: &str) -> Model {
    Model {
        name: name.to_owned(),
        path: path.to_owned(),
        language: ModelLanguage::Sql,
        publication: Publication::Intermediate,
    }
}

/// A project with the model `stg_orders`, the source `erp` with the table `orders`, and the
/// seed `countries`.
fn project() -> (Layout, Configuration) {
    let layout = Layout {
        models: vec![model("stg_orders", "models/staging/stg_orders.sql")],
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

/// The tables the models of [`project`] may read.
fn known() -> KnownTables {
    let (layout, configuration) = project();
    KnownTables::new(&layout, &configuration)
}

/// The codes of the problems of the fixture `name`, read as a step at [`PATH`].
fn codes(name: &str) -> Result<Vec<&'static str>, LayoutError> {
    match read_sql_step(PATH, sql(name).as_bytes(), &known(), &parser()) {
        Ok(_) => Ok(Vec::new()),
        Err(LayoutError::Invalid(problems)) => Ok(problems.iter().map(|p| p.code()).collect()),
        Err(other) => Err(other),
    }
}

#[test]
fn mg_req_0601_sql_model_is_one_query() {
    // `SELECT`, `WITH … SELECT` and `FROM …` are queries.
    for name in ["model", "with_select", "from_first"] {
        assert_eq!(codes(name), Ok(Vec::new()), "{name}");
    }
    // Any other statement, or more than one, is the validation error MG-0314.
    for name in [
        "attach",
        "copy",
        "create",
        "insert",
        "pragma",
        "set",
        "two_statements",
    ] {
        assert_eq!(codes(name), Ok(vec!["MG-0314"]), "{name}");
    }
}

/// The resolved references of each model of `models`, by name and fixture.
fn references(models: &[(&str, &str)]) -> Result<BTreeMap<String, Vec<String>>, LayoutError> {
    let fs = SimFileSystem::new();
    let (_, configuration) = project();
    let mut layout = Layout { models: Vec::new() };
    for (name, fixture) in models {
        let path = format!("models/staging/{name}.sql");
        let full = Path::new("/project").join(&path);
        fs.create_dir_all(Path::new("/project/models/staging"))?;
        fs.write(&full, sql(fixture).as_bytes())?;
        layout.models.push(model(name, &path));
    }
    read_sql_references(
        &fs,
        Path::new("/project"),
        &layout,
        &configuration,
        &parser(),
    )
}

#[test]
fn mg_req_0602_inputs_deduced_and_cte_shadows() {
    let found = references(&[
        ("stg_orders", "source_table"),
        ("a_reads_model", "model"),
        ("b_shadows", "cte_shadows_model"),
        ("c_only_cte", "shadowing_cte"),
    ])
    .unwrap();
    let owned =
        |names: &[&str]| -> Vec<String> { names.iter().map(|name| (*name).to_owned()).collect() };
    // The inputs are the relations the parser finds, not a declared list.
    assert_eq!(found["stg_orders"], owned(&["raw.erp__orders"]));
    assert_eq!(found["a_reads_model"], owned(&["stg_orders"]));
    // A CTE named `stg_orders` shadows the model inside its query: the model is no input.
    assert_eq!(
        found["b_shadows"],
        owned(&["raw.erp__orders", "raw.seed__countries"])
    );
    assert_eq!(found["c_only_cte"], owned(&[]));
}
