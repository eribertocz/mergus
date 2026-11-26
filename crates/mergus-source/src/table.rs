//! The declared tables of a source (MG-REQ-0402) and the raw tables they land in
//! (MG-REQ-0403).

use mergus_project::Source;
use serde_json::Value;

/// A table declared under `[tables.<name>]` of a source file (MG-REQ-0402).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredTable {
    /// The table name, the key of its section.
    pub name: String,
    /// The columns of its `key`, in their declared order; empty when it declares none.
    pub key: Vec<String>,
    /// Its `incremental` column; `None` for a table loaded completely (MG-REQ-0411).
    pub incremental: Option<String>,
}

/// The raw tables a declared table lands in (MG-REQ-0403).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawTables {
    /// `raw.<source>__<table>`: the current rows.
    pub current: String,
    /// `raw.<source>__<table>__changes`: every version and detected delete.
    pub changes: String,
}

impl DeclaredTable {
    /// Returns the raw tables this table of the source `source_name` lands in (MG-REQ-0403).
    #[must_use]
    pub fn raw(&self, source_name: &str) -> RawTables {
        let current = format!("raw.{source_name}__{}", self.name);
        RawTables {
            changes: format!("{current}__changes"),
            current,
        }
    }
}

/// Returns the tables declared by `source`, in the order of their names, read from a
/// configuration that `mergus-project` has validated. Only these are ever extracted
/// (MG-REQ-0402).
#[must_use]
pub fn declared_tables(source: &Source) -> Vec<DeclaredTable> {
    let Some(tables) = source.file.value.get("tables").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut declared: Vec<DeclaredTable> = tables
        .iter()
        .map(|(name, options)| DeclaredTable {
            name: name.clone(),
            key: key_columns(options.get("key")),
            incremental: options
                .get("incremental")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
        .collect();
    declared.sort_by(|left, right| left.name.cmp(&right.name));
    declared
}

/// The columns of a `key`: one column name or a list of them.
fn key_columns(key: Option<&Value>) -> Vec<String> {
    match key {
        Some(Value::String(column)) => vec![column.clone()],
        Some(Value::Array(columns)) => columns
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}
