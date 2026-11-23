//! `mergus new model` and `mergus add source`: one file with a minimal example, in the
//! project found from the current directory (MG-REQ-1304, MG-REQ-1350).

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{ConfigKind, is_valid_name, suggest_valid_name};

use crate::args::Invocation;
use crate::error::CliError;
use crate::files::{Entry, Plan, toml_string};
use crate::project;
use crate::run::Ports;

/// The SQL of a new model (MG-REQ-1350.1).
const SQL: &str = "SELECT 1 AS id\n";

/// The plan of `mergus new model`: `models/<name>.sql`, or under `models/gold/<schema>/` with
/// `--gold`, or a `.py` file with `--py`.
///
/// # Errors
///
/// Fails when no project is found, when the name or the schema is not valid, when the file
/// exists or another model has the name.
pub(crate) fn new_model(ports: &Ports<'_>, invocation: &Invocation) -> Result<Plan, CliError> {
    let root = project::find(ports.environment, ports.files)?;
    let name = valid("model", invocation.value("name").unwrap_or_default())?;
    let folder = match invocation.value("--gold") {
        Some(schema) => format!("models/gold/{}", valid("schema", schema)?),
        None => "models".to_owned(),
    };
    let (extension, text) = if invocation.flag("--py") {
        ("py", python(name))
    } else {
        ("sql", SQL.to_owned())
    };
    let path = format!("{folder}/{name}.{extension}");
    let plan = Plan {
        entries: vec![Entry::File(path, text)],
        root,
    };
    plan.check(ports.files)?;
    if let Some(path) = model_named(ports.files, &plan.root, name)? {
        return Err(CliError::NameTaken {
            name: name.to_owned(),
            path,
        });
    }
    Ok(plan)
}

/// The plan of `mergus add source`: `sources/<name>.toml`, its connection a secret reference.
///
/// # Errors
///
/// Fails when no project is found, when the name is not valid or the file exists.
pub(crate) fn add_source(ports: &Ports<'_>, invocation: &Invocation) -> Result<Plan, CliError> {
    let root = project::find(ports.environment, ports.files)?;
    let name = valid("source", invocation.value("name").unwrap_or_default())?;
    let text = format!(
        "#:schema {}\nkind = \"database\"\nconnection = {}\n",
        ConfigKind::Source.schema_id(),
        toml_string(&format!("secret:{name}"))
    );
    let plan = Plan {
        entries: vec![Entry::File(format!("sources/{name}.toml"), text)],
        root,
    };
    plan.check(ports.files)?;
    Ok(plan)
}

/// Returns `name` when it is a valid name of `kind` (MG-REQ-0305).
///
/// # Errors
///
/// Fails with [`CliError::InvalidName`], suggesting a valid name.
pub(crate) fn valid<'a>(kind: &'static str, name: &'a str) -> Result<&'a str, CliError> {
    if is_valid_name(name) {
        Ok(name)
    } else {
        Err(CliError::InvalidName {
            kind,
            name: name.to_owned(),
            suggestion: suggest_valid_name(name),
        })
    }
}

/// A Python model: one function named `name` returning a one-column Polars data frame.
fn python(name: &str) -> String {
    format!(
        "import polars as pl\n\n\ndef {name}() -> pl.DataFrame:\n    \
         return pl.DataFrame({{\"id\": [1]}})\n"
    )
}

/// The path, relative to `root`, of a model file under `models/` named `name`. An entry is a
/// folder when the port can list it, as `mergus-project` walks it.
fn model_named(
    files: &dyn FileSystem,
    root: &Path,
    name: &str,
) -> Result<Option<String>, CliError> {
    let unreadable = |error: mergus_ports::PortError| CliError::Unreadable(error.to_string());
    let models = root.join("models");
    if !files.exists(&models).map_err(unreadable)? {
        return Ok(None);
    }
    let mut folders = vec![models];
    while let Some(folder) = folders.pop() {
        for entry in files.list_dir(&folder).map_err(unreadable)? {
            if files.list_dir(&entry).is_ok() {
                folders.push(entry);
            } else if entry.file_stem().is_some_and(|stem| stem == name)
                && entry
                    .extension()
                    .is_some_and(|extension| extension == "sql" || extension == "py")
            {
                let relative = entry.strip_prefix(root).unwrap_or(&entry);
                let parts: Vec<String> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                return Ok(Some(parts.join("/")));
            }
        }
    }
    Ok(None)
}
