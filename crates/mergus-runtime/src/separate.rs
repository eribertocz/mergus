//! raw and gold are separate stores (MG-REQ-0501).

use std::path::Path;

use mergus_ports::{FileSystem, PortError};
use mergus_project::{Configuration, key_line};

use crate::{Catalog, Profile, StorageProblem, Zone, ZoneStorage, zone_storage};

/// Checks that raw and gold of `configuration`, a project at `root`, are separate stores
/// (MG-REQ-0501): with `local` or `shared` their locations differ and neither lies inside the
/// other (MG-REQ-0501.1); with `cloud` their catalog secrets differ and neither location
/// prefix holds the other (MG-REQ-0501.2). `project.toml` is read through `files` only to
/// locate each problem on its line. Problems are ordered by line.
///
/// # Errors
///
/// Fails when `project.toml` cannot be read again.
pub fn check_storage(
    files: &dyn FileSystem,
    root: &Path,
    configuration: &Configuration,
) -> Result<Vec<StorageProblem>, PortError> {
    let zones = zone_storage(configuration);
    let find = |zone: Zone| zones.iter().find(|storage| storage.zone == zone);
    let (Some(raw), Some(gold)) = (find(Zone::Raw), find(Zone::Gold)) else {
        return Ok(Vec::new());
    };
    let path = &configuration.project.path;
    let bytes = files.read(&root.join(path))?;
    let text = String::from_utf8_lossy(&bytes);
    let problem = |key: &'static str, value: &str| StorageProblem::SharedStore {
        path: path.clone(),
        line: key_line(&text, &["gold", key]).unwrap_or(1),
        key,
        value: value.to_owned(),
    };
    let mut problems = Vec::new();
    let cloud = |storage: &ZoneStorage| storage.profile == Profile::Cloud;
    if cloud(raw) == cloud(gold)
        && let Some(value) = overlap(&raw.location, &gold.location)
    {
        problems.push(problem("location", value));
    }
    if let (Catalog::Postgres { secret: first }, Catalog::Postgres { secret: second }) =
        (&raw.catalog, &gold.catalog)
        && first == second
    {
        problems.push(problem("catalog", first));
    }
    problems.sort_by_key(StorageProblem::line);
    Ok(problems)
}

/// The location of `raw` or `gold` that holds the other one, or `None` when neither does;
/// `raw` when they are the same.
fn overlap<'a>(raw: &'a str, gold: &'a str) -> Option<&'a str> {
    let (raw_parts, gold_parts) = (parts(raw), parts(gold));
    if gold_parts.starts_with(&raw_parts) {
        Some(raw)
    } else if raw_parts.starts_with(&gold_parts) {
        Some(gold)
    } else {
        None
    }
}

/// The components of a location: its parts between `/` or `\`, without empty and `.` parts,
/// so that `lake`, `./lake/` and `lake//` are the same location.
fn parts(location: &str) -> Vec<&str> {
    location
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect()
}
