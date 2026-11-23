//! Tests of the rejection of reference cycles between models (MG-REQ-0375). The trees are
//! written by hand in the shape `json_serialize_sql` gives: only the nodes the walk reads.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{
    ConfigFile, Configuration, Layout, LayoutError, Model, ModelLanguage, ProjectError,
    Publication, read_sql_references,
};
use mergus_sim::{SimFileSystem, SimSqlParser};
use serde_json::{Value, json};

/// The path of the model `name`.
fn path(name: &str) -> String {
    format!("models/staging/{name}.sql")
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

/// The text and tree of a model that reads `tables`, one per line after `SELECT *` and
/// `blank` empty lines, joined with `JOIN`.
fn model_sql(tables: &[&str], blank: usize) -> (String, String) {
    let mut text = format!("SELECT *\n{}FROM ", "\n".repeat(blank));
    let mut from: Option<Value> = None;
    for (index, table) in tables.iter().enumerate() {
        if index > 0 {
            text.push_str("\nJOIN ");
        }
        let node = base_table(table, text.len());
        text.push_str(table);
        from = Some(match from {
            None => node,
            Some(left) => json!({"type": "JOIN", "left": left, "right": node}),
        });
    }
    let tree = json!({
        "error": false,
        "statements": [{"node": {
            "type": "SELECT_NODE",
            "cte_map": {"map": []},
            "from_table": from,
        }}],
    });
    (text, tree.to_string())
}

/// Reads the references of a project whose `.sql` models are `models`: each a name, the
/// tables it reads and the number of blank lines before its `FROM`.
fn read(models: &[(&str, &[&str], usize)]) -> Result<Vec<String>, LayoutError> {
    let fs = SimFileSystem::new();
    let parser = SimSqlParser::new();
    let mut layout = Layout { models: Vec::new() };
    for (name, tables, blank) in models {
        let (text, tree) = model_sql(tables, *blank);
        let full = Path::new("/project").join(path(name));
        if let Some(parent) = full.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&full, text.as_bytes())?;
        parser.record(text, tree);
        layout.models.push(Model {
            name: (*name).to_owned(),
            path: path(name),
            language: ModelLanguage::Sql,
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
    let found = read_sql_references(&fs, Path::new("/project"), &layout, &configuration, &parser)?;
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
        ("stg_b", &["stg_a"], 2),
        ("stg_a", &["stg_b"], 0),
        ("stg_c", &["stg_a"], 0),
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
        ("m_b", &["m_d"], 0),
        ("m_c", &["m_b"], 1),
        ("m_d", &["m_a", "m_c"], 0),
        ("m_a", &[], 0),
        ("m_e", &["m_c"], 0),
    ]);
    let expected = cycle(&[("m_b", 2), ("m_d", 3), ("m_c", 3)]);
    assert_eq!(found, Err(LayoutError::Invalid(vec![expected])));

    // A model that reads itself, whatever the case of the reference.
    let found = read(&[("cyc", &["CYC"], 0), ("ok", &["cyc"], 0)]);
    assert_eq!(found, Err(LayoutError::Invalid(vec![cycle(&[("cyc", 2)])])));

    // Models without a cycle are listed.
    let found = read(&[("x", &["y", "z"], 0), ("y", &["z"], 0), ("z", &[], 0)]);
    assert_eq!(
        found,
        Ok(vec!["x".to_owned(), "y".to_owned(), "z".to_owned()])
    );
}

#[test]
fn mg_req_0375_cycle_reported_with_other_problems() {
    let Err(LayoutError::Invalid(problems)) =
        read(&[("p", &["q"], 0), ("q", &["p", "nowhere"], 0)])
    else {
        panic!("a cycle and an unknown table are problems");
    };
    let codes: Vec<&str> = problems.iter().map(ProjectError::code).collect();
    assert_eq!(codes, ["MG-0313", "MG-0315"]);
}
