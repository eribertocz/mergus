//! Tests of the rejection of reference cycles between models (MG-REQ-0375). The trees are
//! written by hand in the shape `json_serialize_sql` gives: only the nodes the walk reads, so
//! a word in a comment or a string literal is no table, as in DuckDB's tree.

use std::path::Path;

use mergus_ports::FileSystem;
use std::collections::BTreeMap;

use mergus_project::{
    ConfigFile, Configuration, InputSource, Layout, LayoutError, Model, ModelLanguage,
    ProjectError, Publication, PythonModel, PythonStep, StepEngine, StepInput,
    read_model_references,
};
use mergus_sim::{SimFileSystem, SimSqlParser};
use serde_json::{Value, json};

/// The path of the model `name`: a `.py` model when its name starts with `py_`.
fn path(name: &str) -> String {
    let extension = if name.starts_with("py_") { "py" } else { "sql" };
    format!("models/staging/{name}.{extension}")
}

/// The `.py` model `name` of one step named like it, whose parameters are `inputs`: each a
/// model it reads and the line of the parameter.
fn python(name: &str, inputs: &[(&str, usize)]) -> PythonModel {
    PythonModel {
        path: path(name),
        name: name.to_owned(),
        steps: vec![PythonStep {
            name: name.to_owned(),
            line: 1,
            engine: StepEngine::Polars,
            inputs: inputs
                .iter()
                .map(|(model, line)| StepInput {
                    parameter: (*model).to_owned(),
                    line: *line,
                    source: InputSource::Model((*model).to_owned()),
                })
                .collect(),
        }],
        lib_inputs: BTreeMap::new(),
    }
}

/// A base-table node of `table` at the byte offset `offset`.
fn base_table(table: &str, offset: usize) -> Value {
    json!({
        "type": "BASE_TABLE",
        "query_location": offset,
        "schema_name": "",
        "table_name": table,
        "catalog_name": "",
    })
}

/// The tree of the model text `text`: the tables after each `FROM ` and `JOIN `, joined.
fn tree(text: &str) -> String {
    let mut from: Option<Value> = None;
    for keyword in ["FROM ", "JOIN "] {
        for (offset, _) in text.match_indices(keyword) {
            let start = offset + keyword.len();
            let table = text[start..].split_whitespace().next().unwrap_or("");
            let node = base_table(table, start);
            from = Some(match from {
                None => node,
                Some(left) => json!({"type": "JOIN", "left": left, "right": node}),
            });
        }
    }
    json!({
        "error": false,
        "statements": [{"node": {
            "type": "SELECT_NODE",
            "cte_map": {"map": []},
            "from_table": from,
        }}],
    })
    .to_string()
}

/// Reads the references of a project whose `.sql` models are `models`: each a name and its
/// text.
fn read(models: &[(&str, &str)]) -> Result<Vec<String>, LayoutError> {
    read_with(models, &[])
}

/// Reads the references of a project whose `.sql` models are `models`, each a name and its
/// text, and whose `.py` models are `python`.
fn read_with(models: &[(&str, &str)], python: &[PythonModel]) -> Result<Vec<String>, LayoutError> {
    let fs = SimFileSystem::new();
    let parser = SimSqlParser::new();
    let mut layout = Layout { models: Vec::new() };
    for (name, text) in models {
        let full = Path::new("/project").join(path(name));
        if let Some(parent) = full.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&full, text.as_bytes())?;
        parser.record(*text, tree(text));
        layout.models.push(Model {
            name: (*name).to_owned(),
            path: path(name),
            language: ModelLanguage::Sql,
            publication: Publication::Intermediate,
        });
    }
    for model in python {
        layout.models.push(Model {
            name: model.name.clone(),
            path: model.path.clone(),
            language: ModelLanguage::Python,
            publication: Publication::Intermediate,
        });
    }
    let value = json!({"name": "shop"});
    let configuration = Configuration {
        project: ConfigFile {
            path: "project.toml".to_owned(),
            canonical: mergus_contracts::canonical_json(&value),
            value,
        },
        sources: Vec::new(),
    };
    let root = Path::new("/project");
    let found = read_model_references(&fs, root, &layout, &configuration, &parser, python)?;
    Ok(found.into_keys().collect())
}

/// The cycle error of `models`, each with the line of its reference to the next.
fn cycle(models: &[(&str, usize)]) -> ProjectError {
    ProjectError::ModelCycle {
        models: models.iter().map(|(name, _)| (*name).to_owned()).collect(),
        references: models
            .iter()
            .map(|(name, line)| (path(name), *line))
            .collect(),
    }
}

#[test]
fn mg_req_0375_model_cycle_rejected() {
    // Two models: named from the first in name order, each with the line of its reference.
    let found = read(&[
        ("stg_b", "SELECT *\n\n\nFROM stg_a"),
        ("stg_a", "SELECT *\nFROM stg_b"),
        ("stg_c", "SELECT *\nFROM stg_a"),
    ]);
    let expected = cycle(&[("stg_a", 2), ("stg_b", 4)]);
    assert_eq!(found, Err(LayoutError::Invalid(vec![expected.clone()])));
    assert_eq!(expected.code(), "MG-0315");
    assert_eq!(expected.path(), "models/staging/stg_a.sql");
    assert_eq!(expected.line(), Some(2));
    assert_eq!(
        expected.to_string(),
        "models `stg_a` -> `stg_b` -> `stg_a` form a cycle: \
         `models/staging/stg_a.sql` line 2 reads `stg_b`; \
         `models/staging/stg_b.sql` line 4 reads `stg_a`"
    );

    // A longer cycle, in reference order, with a model that reads into it from outside.
    let found = read(&[
        ("m_b", "SELECT *\nFROM m_d"),
        ("m_c", "SELECT *\n\nFROM m_b"),
        ("m_d", "SELECT *\nFROM m_a\nJOIN m_c"),
        ("m_a", "SELECT 1"),
        ("m_e", "SELECT *\nFROM m_c"),
    ]);
    let expected = cycle(&[("m_b", 2), ("m_d", 3), ("m_c", 3)]);
    assert_eq!(found, Err(LayoutError::Invalid(vec![expected])));

    // A model that reads itself, whatever the case of the reference.
    let found = read(&[("cyc", "SELECT *\nFROM CYC"), ("ok", "SELECT *\nFROM cyc")]);
    assert_eq!(found, Err(LayoutError::Invalid(vec![cycle(&[("cyc", 2)])])));

    // A cycle through a `.py` model, found in the same pass: the line of a `.py` reference
    // is the line of its parameter, not of its `def`.
    let found = read_with(
        &[("a", "SELECT *\n\nFROM py_b")],
        &[python("py_b", &[("a", 4)])],
    );
    let expected = cycle(&[("a", 3), ("py_b", 4)]);
    assert_eq!(found, Err(LayoutError::Invalid(vec![expected])));

    // A comment and a string literal that name the next model before the real reference are
    // no reference: the line is the one of the parsed table name.
    let found = read(&[
        ("n_a", "-- reads n_b\nSELECT 'n_b' AS note\nFROM n_b"),
        ("n_b", "SELECT *\nFROM n_a"),
    ]);
    let expected = cycle(&[("n_a", 3), ("n_b", 2)]);
    assert_eq!(found, Err(LayoutError::Invalid(vec![expected])));

    // Models without a cycle are listed, `.py` models among them.
    let found = read_with(
        &[
            ("x", "SELECT *\nFROM y\nJOIN z"),
            ("y", "SELECT *\nFROM z"),
            ("z", "SELECT 1"),
        ],
        &[python("py_w", &[("x", 2)])],
    );
    assert_eq!(
        found,
        Ok(vec![
            "py_w".to_owned(),
            "x".to_owned(),
            "y".to_owned(),
            "z".to_owned()
        ])
    );
}

#[test]
fn mg_req_0375_cycle_reported_with_other_problems() {
    let Err(LayoutError::Invalid(problems)) = read(&[
        ("p", "SELECT *\nFROM q"),
        ("q", "SELECT *\nFROM p\nJOIN nowhere"),
    ]) else {
        panic!("a cycle and an unknown table are problems");
    };
    let codes: Vec<&str> = problems.iter().map(ProjectError::code).collect();
    assert_eq!(codes, ["MG-0313", "MG-0315"]);
}
