//! Tests of `.py` models (MG-REQ-0612 to MG-REQ-0616, MG-REQ-0618). The trees of `fixtures/`
//! were recorded from the real `Python` port: `python_tree.py` of `mergus-ports` run on each
//! `.py` file with Python 3.

mod eager;
mod lib_modules;
mod returns;
mod steps;
mod wiring;

use mergus_project::{
    ConfigFile, Configuration, KnownTables, Layout, LayoutError, Model, ModelLanguage,
    ProjectError, Publication, PythonLib, PythonModel, PythonModelError, Source, read_python_model,
};
use mergus_sim::SimPython;
use serde_json::{Value, json};

/// The text and the recorded tree of the fixture `name` of `fixtures/`.
macro_rules! fixture {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!("fixtures/", $name, ".py")),
            include_str!(concat!("fixtures/", $name, ".json")),
        )
    };
}
pub(crate) use fixture;

/// Every model fixture: its name, its text and the tree the real port gives for it.
const MODELS: &[(&str, &str, &str)] = &[
    fixture!("async_step"),
    fixture!("eager"),
    fixture!("miswired"),
    fixture!("nameless"),
    fixture!("plain"),
    fixture!("returns"),
    fixture!("sales_by_segment"),
    fixture!("uses_lib"),
    fixture!("wiring"),
];

/// An interpreter that knows the tree of every fixture of `fixtures`.
fn python(fixtures: &[(&str, &str, &str)]) -> SimPython {
    let python = SimPython::new();
    for (_, text, tree) in fixtures {
        python.record(*text, *tree);
    }
    python
}

fn config(path: &str, value: Value) -> ConfigFile {
    ConfigFile {
        path: path.to_owned(),
        canonical: mergus_contracts::canonical_json(&value),
        value,
    }
}

/// The known tables of these tests: the models `stg_orders`, `fact_sales`, `abc`, `abd`,
/// and every fixture model; the raw tables `erp__orders` and `erp__rates`; the seed
/// `countries`.
fn known() -> KnownTables {
    let mut names = vec!["stg_orders", "fact_sales", "abc", "abd"];
    names.extend(MODELS.iter().map(|(name, _, _)| *name));
    let layout = Layout {
        models: names
            .into_iter()
            .map(|name| Model {
                name: name.to_owned(),
                path: format!("models/{name}.py"),
                language: ModelLanguage::Python,
                publication: Publication::Intermediate,
            })
            .collect(),
    };
    let configuration = Configuration {
        project: config(
            "project.toml",
            json!({"name": "shop", "seeds": {"countries": {}}}),
        ),
        sources: vec![Source {
            name: "erp".to_owned(),
            file: config(
                "sources/erp.toml",
                json!({"tables": {"orders": {}, "rates": {}}}),
            ),
        }],
    };
    KnownTables::new(&layout, &configuration)
}

/// Reads the model fixture `name` as `models/<name>.py`, with no `lib` module.
fn read(name: &str) -> Result<PythonModel, LayoutError> {
    let found = MODELS.iter().find(|(fixture, _, _)| *fixture == name);
    assert!(found.is_some(), "no fixture `{name}`");
    let text = found.map_or("", |(_, text, _)| *text);
    let path = format!("models/{name}.py");
    read_python_model(
        &path,
        text.as_bytes(),
        &known(),
        &PythonLib::default(),
        &python(MODELS),
    )
}

/// The problems of the model fixture `name`, which must not read.
fn problems(name: &str) -> Vec<PythonModelError> {
    let read = read(name);
    assert!(
        matches!(&read, Err(LayoutError::Invalid(found))
            if found.iter().all(|problem| matches!(problem, ProjectError::Python(_)))),
        "{name}: expected Python problems, got {read:?}"
    );
    let Err(LayoutError::Invalid(found)) = read else {
        return Vec::new();
    };
    found
        .into_iter()
        .filter_map(|problem| match problem {
            ProjectError::Python(problem) => Some(problem),
            _ => None,
        })
        .collect()
}

/// The code, line and message of each problem.
fn summary(problems: &[PythonModelError]) -> Vec<(&'static str, Option<usize>, String)> {
    problems
        .iter()
        .map(|problem| (problem.code(), problem.line(), problem.to_string()))
        .collect()
}
