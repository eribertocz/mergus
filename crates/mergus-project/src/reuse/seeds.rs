//! Seeds: each `seeds/<name>.csv` declared in `project.toml` with its column types, hashed
//! for the fingerprints of its readers (MG-REQ-0645), and small (MG-REQ-0646).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mergus_contracts::hash::sha256_hex;
use mergus_ports::{FileSystem, ResultColumn, is_column_type};
use serde_json::Value;

use crate::config::key_order;
use crate::{Configuration, LayoutError, ProjectError, ProjectWarning, key_line};

/// The size above which a seed warns, in bytes (MG-REQ-0646.1).
pub const LARGE_SEED_BYTES: u64 = 10_000_000;

/// The path of the file that declares the seeds.
const PROJECT: &str = "project.toml";

/// A declared seed whose CSV file exists (MG-REQ-0645).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    /// The seed name: the stem of `seeds/<name>.csv`.
    pub name: String,
    /// The path of the CSV file, relative to the project root.
    pub path: String,
    /// Its columns, in the order `project.toml` writes them, each with its DuckDB type: a
    /// file without a header is read by position.
    pub columns: Vec<ResultColumn>,
    /// The one character between values.
    pub delimiter: String,
    /// Whether the first line holds the column names.
    pub header: bool,
    /// `sha256:<hex>`, the SHA-256 of the bytes of the file (MG-REQ-0645.3).
    pub hash: String,
}

impl Seed {
    /// The table the seed is loaded as, as a reference names it: `raw.seed__<name>`.
    #[must_use]
    pub fn table(&self) -> String {
        format!("raw.seed__{}", self.name)
    }
}

/// The seeds of a project, the problems of their declarations and the warnings about them.
/// Warnings never fail the read (MG-REQ-0646.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seeds {
    /// The valid seeds, by name.
    pub seeds: Vec<Seed>,
    /// Every problem found: the CSV files of `seeds/` in sorted order, then the declarations
    /// in name order.
    pub problems: Vec<ProjectError>,
    /// Every warning, in the sorted order of the files.
    pub warnings: Vec<ProjectWarning>,
}

impl Seeds {
    /// Returns the fingerprint inputs of the seeds that a model with the resolved
    /// `references` reads: `raw.seed__<name>` to the seed's hash (MG-REQ-0645.3).
    #[must_use]
    pub fn fingerprint_inputs(&self, references: &[String]) -> BTreeMap<String, String> {
        self.seeds
            .iter()
            .map(|seed| (seed.table(), seed.hash.clone()))
            .filter(|(table, _)| references.contains(table))
            .collect()
    }
}

/// Reads the seeds of the project at `root`, whose valid `configuration` declares them
/// (MG-REQ-0645, MG-REQ-0646).
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when a file cannot be read. Problems of the seeds are in
/// [`Seeds::problems`].
pub fn read_seeds(
    files: &dyn FileSystem,
    root: &Path,
    configuration: &Configuration,
) -> Result<Seeds, LayoutError> {
    let declared: BTreeMap<&String, &Value> = configuration
        .project
        .value
        .get("seeds")
        .and_then(Value::as_object)
        .map(|seeds| seeds.iter().collect())
        .unwrap_or_default();
    let project = String::from_utf8_lossy(&files.read(&root.join(PROJECT))?).into_owned();
    let mut read = Seeds::default();
    let mut found = BTreeSet::new();
    for name in csv_stems(files, &root.join("seeds"))? {
        let path = format!("seeds/{name}.csv");
        let Some(declaration) = declared.get(&name) else {
            read.problems
                .push(ProjectError::UndeclaredSeed { name, path });
            continue;
        };
        found.insert(name.clone());
        let bytes = files.read(&root.join(&path))?;
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if size > LARGE_SEED_BYTES {
            read.warnings.push(ProjectWarning::LargeSeed {
                path: path.clone(),
                bytes: size,
            });
        }
        let problems = column_problems(&project, &name, declaration);
        if problems.is_empty() {
            let order = key_order(&project, &["seeds", &name, "columns"]).unwrap_or_default();
            read.seeds
                .push(seed(name, path, declaration, &order, &bytes));
        }
        read.problems.extend(problems);
    }
    for name in declared
        .keys()
        .filter(|name| !found.contains(name.as_str()))
    {
        read.problems.push(ProjectError::SeedWithoutFile {
            name: (*name).clone(),
            path: PROJECT.to_owned(),
            line: key_line(&project, &["seeds", name]).unwrap_or(1),
        });
    }
    Ok(read)
}

/// The stems of the `*.csv` files directly in `folder`, in the port's sorted order.
fn csv_stems(files: &dyn FileSystem, folder: &Path) -> Result<Vec<String>, LayoutError> {
    if !files.exists(folder)? {
        return Ok(Vec::new());
    }
    let mut stems = Vec::new();
    for entry in files.list_dir(folder)? {
        let is_csv = entry.extension().is_some_and(|found| found == "csv");
        if is_csv
            && files.list_dir(&entry).is_err()
            && let Some(stem) = entry.file_stem().and_then(|stem| stem.to_str())
        {
            stems.push(stem.to_owned());
        }
    }
    Ok(stems)
}

/// The columns of `declaration` whose type is not a DuckDB column type name
/// (MG-REQ-0645.1), in name order.
fn column_problems(project: &str, name: &str, declaration: &Value) -> Vec<ProjectError> {
    columns(declaration)
        .filter(|(_, data_type)| !is_column_type(data_type))
        .map(|(column, data_type)| ProjectError::UnknownSeedType {
            path: PROJECT.to_owned(),
            line: key_line(project, &["seeds", name, "columns", column]).unwrap_or(1),
            key: format!("seeds.{name}.columns.{column}"),
            type_name: data_type.to_owned(),
        })
        .collect()
}

/// The columns of a declaration, name to type, in name order.
fn columns(declaration: &Value) -> impl Iterator<Item = (&str, &str)> {
    declaration
        .get("columns")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(column, data_type)| Some((column.as_str(), data_type.as_str()?)))
}

/// The seed `name` at `path`, declared by `declaration` with its columns in `order`, whose
/// file holds `bytes`.
fn seed(name: String, path: String, declaration: &Value, order: &[String], bytes: &[u8]) -> Seed {
    let types: BTreeMap<&str, &str> = columns(declaration).collect();
    Seed {
        columns: order
            .iter()
            .filter_map(|column| {
                Some(ResultColumn {
                    name: column.clone(),
                    data_type: (*types.get(column.as_str())?).to_owned(),
                })
            })
            .collect(),
        delimiter: declaration
            .get("delimiter")
            .and_then(Value::as_str)
            .unwrap_or(",")
            .to_owned(),
        header: declaration
            .get("header")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        hash: format!("sha256:{}", sha256_hex(bytes)),
        name,
        path,
    }
}
