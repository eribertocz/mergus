//! A walk of the tree of `json_serialize_sql` (MG-REQ-0193): its statements (MG-REQ-0374.1),
//! table functions (MG-REQ-0374.2), base tables (MG-REQ-0374.3, MG-REQ-0373) and functions
//! (MG-REQ-0374.4).

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{KnownTables, Position, forbidden};
use crate::ProjectError;
use crate::names::{closest_name, is_valid_name};

/// The only table functions a model may call (MG-REQ-0374.2).
const TABLE_FUNCTIONS: [&str; 3] = ["range", "generate_series", "unnest"];

/// The functions a model may not call (MG-REQ-0374.4).
const FUNCTIONS: [&str; 4] = ["getenv", "nextval", "currval", "setseed"];

/// What a walk finds in a model: its problems, and the references that resolve.
pub(crate) struct Checked {
    /// The problems, in walking order.
    pub(crate) problems: Vec<ProjectError>,
    /// The references that resolve, lowercase: a model bare, a raw table as `raw.<table>`;
    /// each with the position of its first occurrence in the text.
    pub(crate) references: BTreeMap<String, Position>,
    /// The named parameters of its statements, in the order DuckDB numbers them
    /// (MG-REQ-0603).
    pub(crate) parameters: Vec<String>,
}

/// Returns the problems and the resolved references of the tree `tree` of the text `text` of
/// the model at `path`.
pub(crate) fn check(path: &str, text: &str, tree: &Value, known: &KnownTables) -> Checked {
    let mut walk = Walk {
        path,
        text,
        known,
        ctes: Vec::new(),
        problems: Vec::new(),
        references: BTreeMap::new(),
    };
    if tree.get("error").and_then(Value::as_bool) != Some(false) {
        walk.parse_error(tree);
        return walk.finish();
    }
    let statements = tree
        .get("statements")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    match statements {
        [] => walk.problem(None, "no statement".to_owned()),
        [_] => {}
        [_, second, ..] => {
            let at = walk.first_location(second);
            walk.problem(at, format!("{} statements", statements.len()));
        }
    }
    for statement in statements {
        walk.value(statement, None);
    }
    let mut checked = walk.finish();
    checked.parameters = parameters(statements);
    checked
}

/// The named parameters of `statements`, from DuckDB's `named_param_map`, ordered by the
/// number DuckDB gives each.
fn parameters(statements: &[Value]) -> Vec<String> {
    let mut numbered: Vec<(u64, &str)> = statements
        .iter()
        .filter_map(|statement| statement.get("named_param_map").and_then(Value::as_array))
        .flatten()
        .filter_map(|entry| {
            let name = entry.get("key").and_then(Value::as_str)?;
            Some((entry.get("value").and_then(Value::as_u64)?, name))
        })
        .collect();
    numbered.sort_unstable();
    numbered
        .into_iter()
        .map(|(_, name)| name.to_owned())
        .collect()
}

/// The state of one walk: the CTEs visible at the current node, lowercase.
struct Walk<'a> {
    path: &'a str,
    text: &'a str,
    known: &'a KnownTables,
    ctes: Vec<String>,
    problems: Vec<ProjectError>,
    references: BTreeMap<String, Position>,
}

impl Walk<'_> {
    /// Ends the walk.
    fn finish(self) -> Checked {
        Checked {
            problems: self.problems,
            references: self.references,
            parameters: Vec::new(),
        }
    }

    /// Records the error the parser reports, at its position when it gives one
    /// (MG-REQ-0374.1).
    fn parse_error(&mut self, tree: &Value) {
        let message = tree
            .get("error_message")
            .and_then(Value::as_str)
            .unwrap_or("no tree");
        let at = tree
            .get("position")
            .and_then(Value::as_str)
            .and_then(|position| position.parse().ok());
        self.problem(
            at,
            format!("a statement DuckDB does not parse as a SELECT ({message})"),
        );
    }

    /// Walks `value`, whose nearest enclosing location is `at`.
    fn value(&mut self, value: &Value, at: Option<usize>) {
        match value {
            Value::Array(items) => items.iter().for_each(|item| self.value(item, at)),
            Value::Object(node) => self.node(node, at),
            _ => {}
        }
    }

    /// Walks one node of the tree. The CTEs of a query node are visible in its body and in
    /// the CTEs defined after them; a recursive CTE also in itself.
    fn node(&mut self, node: &Map<String, Value>, at: Option<usize>) {
        let at = self.location(node).or(at);
        let outer = self.ctes.len();
        if text(node, "type") == "RECURSIVE_CTE_NODE" {
            self.ctes.push(text(node, "cte_name").to_lowercase());
        }
        let ctes = node
            .get("cte_map")
            .and_then(|ctes| ctes.get("map"))
            .and_then(Value::as_array);
        for cte in ctes.into_iter().flatten() {
            if let Some(query) = cte.get("value") {
                self.value(query, at);
            }
            if let Some(name) = cte.get("key").and_then(Value::as_str) {
                self.ctes.push(name.to_lowercase());
            }
        }
        let mut skipped = "cte_map";
        match text(node, "type") {
            "BASE_TABLE" => self.base_table(node, at),
            "TABLE_FUNCTION" => {
                self.table_function(node, at);
                skipped = "function";
            }
            _ if text(node, "class") == "FUNCTION" => self.function(node, at),
            _ => {}
        }
        for (key, child) in node {
            if key != skipped && key != "cte_map" {
                self.value(child, at);
            }
        }
        self.ctes.truncate(outer);
    }

    /// Checks a base-table reference (MG-REQ-0374.3, MG-REQ-0373).
    fn base_table(&mut self, node: &Map<String, Value>, at: Option<usize>) {
        let (catalog, schema, table) = (
            text(node, "catalog_name"),
            text(node, "schema_name"),
            text(node, "table_name"),
        );
        let lowered = table.to_lowercase();
        if catalog.is_empty() && schema.is_empty() && self.ctes.contains(&lowered) {
            return;
        }
        if !is_valid_name(&lowered) {
            self.problem(at, format!("table name `{table}`"));
        } else if self.known.resolves(catalog, schema, table) {
            let name = if schema.is_empty() {
                lowered
            } else {
                format!("{}.{lowered}", schema.to_lowercase())
            };
            let at = self.position(at);
            self.references
                .entry(name)
                .and_modify(|first| *first = (*first).min(at))
                .or_insert(at);
        } else {
            let name: Vec<&str> = [catalog, schema, table]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect();
            let name = name.join(".");
            let names: Vec<String> = self.known.names().collect();
            let suggestion = closest_name(&name.to_lowercase(), names.iter().map(String::as_str))
                .map(str::to_owned);
            let at = self.position(at);
            self.problems.push(ProjectError::UnknownReference {
                path: self.path.to_owned(),
                line: at.line,
                column: at.column,
                name,
                suggestion,
            });
        }
    }

    /// Checks a table function (MG-REQ-0374.2), then walks its arguments.
    fn table_function(&mut self, node: &Map<String, Value>, at: Option<usize>) {
        let Some(function) = node.get("function").and_then(Value::as_object) else {
            return;
        };
        let name = text(function, "function_name");
        if !TABLE_FUNCTIONS.contains(&name.to_lowercase().as_str()) {
            self.problem(at, format!("table function `{name}`"));
        }
        for child in function.values() {
            self.value(child, at);
        }
    }

    /// Checks a function call (MG-REQ-0374.4).
    fn function(&mut self, node: &Map<String, Value>, at: Option<usize>) {
        let name = text(node, "function_name");
        if FUNCTIONS.contains(&name.to_lowercase().as_str()) {
            self.problem(at, format!("function `{name}`"));
        }
    }

    /// The first location of `value` and of the nodes under it.
    fn first_location(&self, value: &Value) -> Option<usize> {
        match value {
            Value::Array(items) => items.iter().filter_map(|i| self.first_location(i)).min(),
            Value::Object(node) => node
                .values()
                .filter_map(|child| self.first_location(child))
                .chain(self.location(node))
                .min(),
            _ => None,
        }
    }

    /// The byte offset of `node` in the text, when the tree gives one inside the text.
    fn location(&self, node: &Map<String, Value>) -> Option<usize> {
        let offset = node.get("query_location").and_then(Value::as_u64)?;
        usize::try_from(offset)
            .ok()
            .filter(|offset| *offset < self.text.len())
    }

    /// The position of the byte offset `at`, or the start of the text.
    fn position(&self, at: Option<usize>) -> Position {
        Position::of(self.text, at.unwrap_or(0))
    }

    /// Records the error of MG-REQ-0374 for `construct` at the byte offset `at`.
    fn problem(&mut self, at: Option<usize>, construct: String) {
        let at = self.position(at);
        self.problems.push(forbidden(self.path, at, construct));
    }
}

/// The text of the key `key` of `node`, empty when it has none.
fn text<'a>(node: &'a Map<String, Value>, key: &str) -> &'a str {
    node.get(key).and_then(Value::as_str).unwrap_or("")
}
