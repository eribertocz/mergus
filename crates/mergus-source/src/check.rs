//! The checks of the source files (MG-REQ-0409, MG-REQ-0414, MG-REQ-0416).

use std::path::Path;

use mergus_ports::{FileSystem, PortError};
use mergus_project::{Configuration, Source, key_line};
use serde_json::{Map, Value};

use crate::{Mode, SourceKind, SourceProblem};

/// Checks every source of `configuration`, a project at `root` whose files are read through
/// `files` only to locate each problem on its line. Problems come source by source, in the
/// order of `configuration`, each source's ordered by line, code and key.
///
/// # Errors
///
/// Fails when a source file cannot be read again.
pub fn check_sources(
    files: &dyn FileSystem,
    root: &Path,
    configuration: &Configuration,
) -> Result<Vec<SourceProblem>, PortError> {
    let mut problems = Vec::new();
    for source in &configuration.sources {
        let bytes = files.read(&root.join(&source.file.path))?;
        let text = String::from_utf8_lossy(&bytes);
        let mut found = check_source(source, &Lines(&text));
        found.sort_by_cached_key(|problem| (problem.line(), problem.code(), problem.key()));
        problems.append(&mut found);
    }
    Ok(problems)
}

/// The text of a source file, which gives the line of its keys.
struct Lines<'a>(&'a str);

impl Lines<'_> {
    /// The line, from 1, of the key at `keys`; 1 when the file does not hold it.
    fn of(&self, keys: &[&str]) -> usize {
        key_line(self.0, keys).unwrap_or(1)
    }
}

/// The problems of one source.
fn check_source(source: &Source, lines: &Lines<'_>) -> Vec<SourceProblem> {
    let Some(object) = source.file.value.as_object() else {
        return Vec::new();
    };
    let kind = object.get("kind").and_then(Value::as_str);
    if kind.and_then(SourceKind::parse).is_none() {
        return Vec::new();
    }
    let mut problems = Vec::new();
    problems.extend(engine(source, object, lines));
    if let Some(tables) = object.get("tables").and_then(Value::as_object) {
        for (table, options) in tables {
            if let Some(options) = options.as_object() {
                problems.extend(table_problems(source, table, options, lines));
            }
        }
    }
    problems
}

/// MG-REQ-0409: the engine of a `connect` or `live` source is in the allowlist of its mode,
/// and a `copy` source declares none.
fn engine(
    source: &Source,
    object: &Map<String, Value>,
    lines: &Lines<'_>,
) -> Option<SourceProblem> {
    let mode = match object.get("mode") {
        None => Mode::Copy,
        Some(mode) => Mode::parse(mode.as_str()?)?,
    };
    let engine = match object.get("engine") {
        None => None,
        Some(engine) => Some(engine.as_str()?.to_owned()),
    };
    let allowed = match (&engine, mode) {
        (None, Mode::Copy) => true,
        (None, _) => false,
        (Some(engine), mode) => mode.engines().contains(&engine.as_str()),
    };
    if allowed {
        return None;
    }
    let key = if engine.is_some() { "engine" } else { "mode" };
    Some(SourceProblem::EngineNotAllowed {
        path: source.file.path.clone(),
        line: lines.of(&[key]),
        source_name: source.name.clone(),
        mode,
        engine,
    })
}

/// MG-REQ-0414 and MG-REQ-0416: an incremental table declares a `key`, and should declare
/// `deletes`.
fn table_problems(
    source: &Source,
    table: &str,
    options: &Map<String, Value>,
    lines: &Lines<'_>,
) -> Vec<SourceProblem> {
    if !options.contains_key("incremental") {
        return Vec::new();
    }
    let line = lines.of(&["tables", table, "incremental"]);
    let mut problems = Vec::new();
    if !options.contains_key("key") {
        problems.push(SourceProblem::IncrementalWithoutKey {
            path: source.file.path.clone(),
            line,
            source_name: source.name.clone(),
            table: table.to_owned(),
        });
    }
    if !options.contains_key("deletes") {
        problems.push(SourceProblem::NoDeletePolicy {
            path: source.file.path.clone(),
            line,
            source_name: source.name.clone(),
            table: table.to_owned(),
        });
    }
    problems
}
