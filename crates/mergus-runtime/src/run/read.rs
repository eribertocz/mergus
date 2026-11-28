//! Stage 1: reads the project and finds every error in one pass, grouped by file
//! (MG-REQ-0701, MG-REQ-0702), a cycle between models among them, which `mergus-project`
//! finds over `.sql` and `.py` models in one pass (MG-REQ-0375, MG-REQ-0703).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mergus_ports::{Database, FileSystem, Python, SqlParser};
use mergus_project::{
    Configuration, Identifier, Identifiers, KnownTables, Layout, LayoutError, Macros,
    ModelLanguage, ModelOptions, ProjectError, PythonModel, Renames, Seeds, read_configuration,
    read_layout, read_macros, read_model_options, read_model_references, read_python_lib,
    read_renames, read_seeds,
};

use super::read_errors::{FileErrors, group};

/// The ports stage 1 reads through. Without `python`, `.py` models are not read; without
/// `database`, `macros/` is not.
#[derive(Clone, Copy)]
pub struct ReadPorts<'a> {
    /// The project's files.
    pub files: &'a dyn FileSystem,
    /// DuckDB's parser, for `.sql` models.
    pub parser: &'a dyn SqlParser,
    /// The Python parser, for `.py` models and `lib/`.
    pub python: Option<&'a dyn Python>,
    /// A database for the sandboxed session of `macros/`; it reads no table data.
    pub database: Option<&'a dyn Database>,
}

/// One model of the read project.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadModel {
    /// Its effective options.
    pub options: ModelOptions,
    /// Its stable identifier.
    pub identifier: Identifier,
    /// The text of its file.
    pub code: String,
    /// The tables it reads: a model bare, a raw table as `raw.<table>`.
    pub inputs: Vec<String>,
}

/// The output of stage 1: the project in canonical form.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadProject {
    /// The project root.
    pub root: PathBuf,
    /// Its layout.
    pub layout: Layout,
    /// `project.toml` and the source files.
    pub configuration: Configuration,
    /// Its models, in walking order.
    pub models: Vec<ReadModel>,
    /// Its macros; empty when `macros/` was not read.
    pub macros: Macros,
    /// Its seeds.
    pub seeds: Seeds,
}

/// Why stage 1 failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// The project has errors: every one found, grouped by file (MG-REQ-0702).
    #[error("the project has errors in {} file(s)", .0.len())]
    Invalid(Vec<FileErrors>),
    /// A port failed, or a bundled schema is broken.
    #[error(transparent)]
    Unreadable(LayoutError),
}

/// Collects the errors of each pass of stage 1.
struct Passes {
    errors: Vec<ProjectError>,
}

impl Passes {
    fn take<T>(&mut self, result: Result<T, LayoutError>) -> Result<Option<T>, ReadError> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(LayoutError::Invalid(errors)) => {
                self.errors.extend(errors);
                Ok(None)
            }
            Err(error) => Err(ReadError::Unreadable(error)),
        }
    }
}

/// Reads the project at `root`: its layout, configuration, identifiers, folder options,
/// `.sql` models with their references and cycles, seeds, `lib/` and `.py` models, and
/// macros. Every pass whose inputs are valid runs, so every error is found at once. Reads
/// no table data (MG-REQ-0701).
///
/// # Errors
///
/// Fails with [`ReadError::Invalid`] carrying every error, grouped by file in layout order
/// (MG-REQ-0702.1), and with [`ReadError::Unreadable`] when a port fails.
pub fn read_project(ports: &ReadPorts<'_>, root: &Path) -> Result<ReadProject, ReadError> {
    let mut passes = Passes { errors: Vec::new() };
    let found = find(ports, root, &mut passes)?;
    let Found {
        layout: Some(layout),
        configuration: Some(configuration),
        options: Some(options),
        identifiers: Some(identifiers),
        references: Some(references),
        seeds: Some(seeds),
        macros: Some(macros),
        ..
    } = found
    else {
        return Err(invalid(passes.errors));
    };
    if !passes.errors.is_empty() {
        return Err(invalid(passes.errors));
    }
    let mut models = Vec::with_capacity(options.len());
    for (options, identifier) in options.into_iter().zip(identifiers) {
        let code = read_text(ports.files, root, &options.path)?;
        let inputs = references.get(&options.name).cloned().unwrap_or_default();
        models.push(ReadModel {
            options,
            identifier,
            code,
            inputs,
        });
    }
    Ok(ReadProject {
        root: root.to_path_buf(),
        layout,
        configuration,
        models,
        macros,
        seeds,
    })
}

/// What the passes of stage 1 found; `None` for a pass that failed or could not run.
#[derive(Default)]
struct Found {
    layout: Option<Layout>,
    configuration: Option<Configuration>,
    options: Option<Vec<ModelOptions>>,
    identifiers: Option<Vec<Identifier>>,
    references: Option<BTreeMap<String, Vec<String>>>,
    seeds: Option<Seeds>,
    macros: Option<Macros>,
}

/// Runs every pass of stage 1 whose inputs are valid, collecting their errors in `passes`.
fn find(ports: &ReadPorts<'_>, root: &Path, passes: &mut Passes) -> Result<Found, ReadError> {
    let files = ports.files;
    let mut found = Found {
        layout: passes.take(read_layout(files, root))?,
        configuration: passes.take(read_configuration(files, root))?,
        ..Found::default()
    };
    let renames = passes.take(read_renames(files, root))?;
    if let Some(layout) = &found.layout {
        found.options = passes.take(read_model_options(files, root, layout))?;
    }
    if let (Some(renames), Some(options)) = (&renames, &found.options) {
        found.identifiers = Some(identifiers(renames, options, passes));
    }
    if let (Some(layout), Some(configuration)) = (&found.layout, &found.configuration) {
        let python = read_python(ports, root, layout, configuration, passes)?;
        let references =
            read_model_references(files, root, layout, configuration, ports.parser, &python);
        found.references = passes.take(references)?;
        if let Some(seeds) = passes.take(read_seeds(files, root, configuration))? {
            passes.errors.extend(seeds.problems.iter().cloned());
            found.seeds = Some(seeds);
        }
    }
    found.macros = match ports.database {
        Some(database) => passes.take(read_macros(files, root, database, ports.parser))?,
        None => Some(Macros::default()),
    };
    Ok(found)
}

/// The identifier of each model of `options`, in order; the collisions go to `passes`
/// (MG-REQ-0371).
fn identifiers(
    renames: &Renames,
    options: &[ModelOptions],
    passes: &mut Passes,
) -> Vec<Identifier> {
    let mut identifiers = Identifiers::new(renames);
    let assigned = options
        .iter()
        .map(|options| identifiers.model(&options.name, &options.path))
        .collect();
    if let Err(errors) = identifiers.finish() {
        passes.errors.extend(errors);
    }
    assigned
}

/// Reads `lib/` and every `.py` model, when the Python port is given, and returns each
/// valid model.
fn read_python(
    ports: &ReadPorts<'_>,
    root: &Path,
    layout: &Layout,
    configuration: &Configuration,
    passes: &mut Passes,
) -> Result<Vec<PythonModel>, ReadError> {
    let Some(python) = ports.python else {
        return Ok(Vec::new());
    };
    let Some(lib) = passes.take(read_python_lib(ports.files, root, python))? else {
        return Ok(Vec::new());
    };
    let known = KnownTables::new(layout, configuration);
    let mut models = Vec::new();
    for model in &layout.models {
        if model.language != ModelLanguage::Python {
            continue;
        }
        let bytes = ports
            .files
            .read(&root.join(&model.path))
            .map_err(|error| ReadError::Unreadable(LayoutError::Port(error)))?;
        let read = mergus_project::read_python_model(&model.path, &bytes, &known, &lib, python);
        if let Some(read) = passes.take(read)? {
            models.push(read);
        }
    }
    Ok(models)
}

/// The text of the file at `path`, relative to `root`.
fn read_text(files: &dyn FileSystem, root: &Path, path: &str) -> Result<String, ReadError> {
    let bytes = files
        .read(&root.join(path))
        .map_err(|error| ReadError::Unreadable(LayoutError::Port(error)))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The [`ReadError::Invalid`] of `errors`, grouped.
fn invalid(errors: Vec<ProjectError>) -> ReadError {
    ReadError::Invalid(group(errors))
}
