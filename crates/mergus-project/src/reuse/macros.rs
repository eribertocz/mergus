//! Macros: DuckDB macros only (MG-REQ-0641), with unique names that shadow no built-in
//! (MG-REQ-0642), fingerprinted by use (MG-REQ-0644).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mergus_contracts::hash::sha256_hex;
use mergus_ports::{CatalogMacro, Database, FileSystem, MacroRun, PortError, SqlParser};
use serde_json::Value;

use crate::{LayoutError, ProjectError};

/// The names of Mergus's own macros, reserved like built-ins (MG-REQ-0642.4).
const RESERVED: [&str; 1] = ["as_of"];

/// The prefix of the fingerprint input of a macro (MG-REQ-0644.2).
const INPUT_PREFIX: &str = "macro:";

/// One macro of the project: every overload of one name in one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Macro {
    /// The name, lowercase.
    pub name: String,
    /// The path of its file, relative to the project root.
    pub path: String,
    /// Whether one of its overloads is a table macro.
    pub table: bool,
    /// Its `macro_definition`; the definitions of its overloads one per line, in the order of
    /// their text (the port sorts them), when it has several.
    pub definition: String,
    /// Its overloads, each parsed in its own form for its calls (MG-REQ-0644.1).
    pub overloads: Vec<CatalogMacro>,
    /// `sha256:<hex>`, the SHA-256 of [`Macro::definition`] (MG-REQ-0644.2).
    pub hash: String,
    /// The macros of the project it calls, lowercase (MG-REQ-0644.1).
    pub calls: BTreeSet<String>,
}

/// The macros of a project, by lowercase name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Macros {
    /// Every macro, by lowercase name.
    pub macros: BTreeMap<String, Macro>,
}

impl Macros {
    /// Returns the fingerprint inputs of a model whose query calls the functions `calls`
    /// (lowercase): `macro:<name>` to the hash of each macro reachable from them, directly or
    /// through another macro (MG-REQ-0644.2).
    #[must_use]
    pub fn fingerprint_inputs(&self, calls: &BTreeSet<String>) -> BTreeMap<String, String> {
        let mut used = BTreeMap::new();
        let mut pending: Vec<&String> = calls.iter().collect();
        while let Some(name) = pending.pop() {
            let Some(found) = self.macros.get(name) else {
                continue;
            };
            let input = format!("{INPUT_PREFIX}{name}");
            if used.insert(input, found.hash.clone()).is_none() {
                pending.extend(found.calls.iter());
            }
        }
        used
    }
}

/// Returns the function names of the tree `parser` gives for `sql`, lowercase
/// (MG-REQ-0644.1).
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when the parser cannot be reached, or does not read
/// `sql`.
pub fn sql_calls(parser: &dyn SqlParser, sql: &str) -> Result<BTreeSet<String>, LayoutError> {
    let tree: Value = serde_json::from_str(&parser.parse(sql)?).map_err(|error| {
        PortError::Failed(format!(
            "the SQL parser returned a tree that is not JSON: {error}"
        ))
    })?;
    if tree.get("error").and_then(Value::as_bool) != Some(false) {
        return Err(LayoutError::Port(PortError::Failed(format!(
            "the SQL parser does not read `{sql}`"
        ))));
    }
    let mut calls = BTreeSet::new();
    function_names(&tree, &mut calls);
    Ok(calls)
}

/// Adds every `function_name` under `value` to `found`, lowercase.
fn function_names(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| function_names(item, found)),
        Value::Object(node) => {
            if let Some(name) = node.get("function_name").and_then(Value::as_str) {
                found.insert(name.to_lowercase());
            }
            node.values().for_each(|child| function_names(child, found));
        }
        _ => {}
    }
}

/// Reads every `macros/*.sql` of the project at `root`, each run by `database` in its own
/// sandboxed session (MG-REQ-0641), and the calls of each macro, parsed by `parser`
/// (MG-REQ-0644.1).
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when a port fails, and with [`LayoutError::Invalid`]
/// carrying every problem found, file by file in sorted order (MG-REQ-0641.2,
/// MG-REQ-0642.3).
pub fn read_macros(
    files: &dyn FileSystem,
    root: &Path,
    database: &dyn Database,
    parser: &dyn SqlParser,
) -> Result<Macros, LayoutError> {
    let mut read = Read::default();
    let folder = root.join("macros");
    if files.exists(&folder)? {
        for entry in files.list_dir(&folder)? {
            let is_sql = entry.extension().is_some_and(|found| found == "sql");
            if !is_sql || files.list_dir(&entry).is_ok() {
                continue;
            }
            let Some(stem) = entry.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let path = format!("macros/{stem}.sql");
            read.file(&path, &files.read(&entry)?, database)?;
        }
    }
    if !read.problems.is_empty() {
        return Err(LayoutError::Invalid(read.problems));
    }
    let names: BTreeSet<String> = read.macros.keys().cloned().collect();
    for found in read.macros.values_mut() {
        let mut calls = BTreeSet::new();
        for overload in &found.overloads {
            // MG-REQ-0644.1: the text is only parsed, never run; a scalar macro's definition
            // is an expression, read as the select list of `SELECT <definition>`.
            let body = if overload.table {
                overload.definition.clone()
            } else {
                ["SELECT ", overload.definition.as_str()].concat()
            };
            calls.extend(sql_calls(parser, &body)?.intersection(&names).cloned());
        }
        found.calls = calls;
    }
    Ok(Macros {
        macros: read.macros,
    })
}

/// The macros and problems found so far.
#[derive(Default)]
struct Read {
    macros: BTreeMap<String, Macro>,
    problems: Vec<ProjectError>,
}

impl Read {
    /// Runs the macro file at `path`, holding `bytes`, and records its macros or problems.
    fn file(&mut self, path: &str, bytes: &[u8], database: &dyn Database) -> Result<(), PortError> {
        let forbidden = |reason: String| ProjectError::ForbiddenMacroFile {
            path: path.to_owned(),
            reason,
        };
        let Ok(text) = std::str::from_utf8(bytes) else {
            self.problems
                .push(forbidden("its text is not UTF-8".to_owned()));
            return Ok(());
        };
        let run = database.run_macros(text)?;
        if let Some(message) = &run.refused {
            let message = message.split("\n\nLINE ").next().unwrap_or(message);
            self.problems.push(forbidden(format!(
                "DuckDB refuses a statement ({})",
                message.trim_end()
            )));
            return Ok(());
        }
        if !run.changed.is_empty() {
            self.problems.push(forbidden(format!(
                "it creates objects other than macros, seen in {}",
                changed_functions(&run)
            )));
            return Ok(());
        }
        self.record(path, &run);
        Ok(())
    }

    /// Records the macros of the file at `path`, which ran as `run`: a name of a built-in or
    /// of Mergus, or one an earlier file has, is a clash (MG-REQ-0642).
    fn record(&mut self, path: &str, run: &MacroRun) {
        let builtins: BTreeMap<String, &str> = run
            .builtins
            .iter()
            .map(String::as_str)
            .chain(RESERVED)
            .map(|name| (name.to_lowercase(), name))
            .collect();
        for found in grouped(path, &run.macros) {
            if let Some(builtin) = builtins.get(&found.name) {
                self.problems.push(ProjectError::MacroShadowsBuiltin {
                    name: found.name.clone(),
                    path: path.to_owned(),
                    builtin: (*builtin).to_owned(),
                });
            } else if let Some(first) = self.macros.get(&found.name) {
                self.problems.push(ProjectError::DuplicateMacro {
                    name: found.name.clone(),
                    first: first.path.clone(),
                    path: path.to_owned(),
                });
            } else {
                self.macros.insert(found.name.clone(), found);
            }
        }
    }
}

/// The catalog functions whose rows the file changed, such as `duckdb_tables()`.
fn changed_functions(run: &MacroRun) -> String {
    let functions: BTreeSet<&str> = run
        .changed
        .iter()
        .filter_map(|row| row.split_once(": ").map(|(function, _)| function))
        .collect();
    functions
        .iter()
        .map(|function| format!("`{function}()`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The macros of one file, the overloads of each lowercase name together, in name order.
fn grouped(path: &str, macros: &[CatalogMacro]) -> Vec<Macro> {
    let mut by_name: BTreeMap<String, Vec<&CatalogMacro>> = BTreeMap::new();
    for found in macros {
        by_name
            .entry(found.name.to_lowercase())
            .or_default()
            .push(found);
    }
    by_name
        .into_iter()
        .map(|(name, overloads)| {
            let definition = overloads
                .iter()
                .map(|found| found.definition.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            Macro {
                name,
                path: path.to_owned(),
                table: overloads.iter().any(|found| found.table),
                hash: format!("sha256:{}", sha256_hex(definition.as_bytes())),
                definition,
                overloads: overloads.into_iter().cloned().collect(),
                calls: BTreeSet::new(),
            }
        })
        .collect()
}
