//! The models of the project as `ls`, `explain` and `plan` need them: their options, their
//! identifier, their code and the tables they read.

use std::collections::BTreeSet;
use std::path::PathBuf;

use mergus_ports::FileSystem;
use mergus_project::{
    Configuration, Effective, Identifier, Identifiers, ModelOptions, read_configuration,
    read_layout, read_model_options, read_renames, read_sql_references,
};

use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::project;
use crate::run::Ports;
use crate::validate::collect;

/// One model of the project.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProjectModel {
    /// Its effective options, with their origins (MG-REQ-0335).
    pub(crate) options: ModelOptions,
    /// Its stable identifier (MG-REQ-0368).
    pub(crate) identifier: Identifier,
    /// The text of its file.
    pub(crate) code: String,
    /// The tables it reads, resolved (MG-REQ-0373): a model bare, a raw table as
    /// `raw.<table>`.
    pub(crate) inputs: Vec<String>,
}

impl ProjectModel {
    /// The model name.
    pub(crate) fn name(&self) -> &str {
        &self.options.name
    }

    /// The path of its file, relative to the project root, separated by `/`.
    pub(crate) fn path(&self) -> &str {
        &self.options.path
    }

    /// Its tags: the items of its effective `tags` option.
    pub(crate) fn tags(&self) -> BTreeSet<String> {
        match self.options.options.get("tags") {
            Some(Effective::List(items)) => items
                .iter()
                .filter_map(|item| item.value.as_str().map(str::to_owned))
                .collect(),
            _ => BTreeSet::new(),
        }
    }
}

/// The project read for `ls`, `explain` and `plan`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Project {
    /// The root of the project.
    pub(crate) root: PathBuf,
    /// The names of its sources, sorted.
    pub(crate) sources: Vec<String>,
    /// Its models, in the walking order of the layout.
    pub(crate) models: Vec<ProjectModel>,
    /// Its sources, as `plan` shows what reading them costs (MG-REQ-0432).
    pub(crate) configuration: Configuration,
}

impl Project {
    /// The model named `name`.
    pub(crate) fn model(&self, name: &str) -> Option<&ProjectModel> {
        self.models.iter().find(|model| model.name() == name)
    }

    /// The names of the models that read the model `name`, sorted.
    pub(crate) fn readers(&self, name: &str) -> Vec<&str> {
        let mut readers: Vec<&str> = self
            .models
            .iter()
            .filter(|model| model.inputs.iter().any(|input| input == name))
            .map(ProjectModel::name)
            .collect();
        readers.sort_unstable();
        readers
    }
}

/// Reads the project found from the current directory: its layout, its configuration, the
/// options, identifiers and references of its models.
///
/// # Errors
///
/// Returns the diagnostics of every problem of `mergus validate` found on the way, as
/// `Ok(Err(..))`; fails when the project cannot be found or read.
pub(crate) fn read(ports: &Ports<'_>) -> Result<Result<Project, Vec<Diagnostic>>, CliError> {
    let root = project::find(ports.environment, ports.files)?;
    let mut diagnostics = Vec::new();
    let layout = collect(read_layout(ports.files, &root), &mut diagnostics)?;
    let configuration = collect(read_configuration(ports.files, &root), &mut diagnostics)?;
    let renames = collect(read_renames(ports.files, &root), &mut diagnostics)?;
    let (Some(layout), Some(configuration), Some(renames)) = (layout, configuration, renames)
    else {
        return Ok(Err(diagnostics));
    };
    let options = collect(
        read_model_options(ports.files, &root, &layout),
        &mut diagnostics,
    )?;
    let references = collect(
        read_sql_references(
            ports.files,
            &root,
            &layout,
            &configuration,
            ports.sql_parser,
        ),
        &mut diagnostics,
    )?;
    let (Some(options), Some(references)) = (options, references) else {
        return Ok(Err(diagnostics));
    };
    let mut identifiers = Identifiers::new(&renames);
    let mut models = Vec::with_capacity(options.len());
    for options in options {
        let identifier = identifiers.model(&options.name, &options.path);
        let code = read_text(ports.files, &root, &options.path)?;
        let inputs = references.get(&options.name).cloned().unwrap_or_default();
        models.push(ProjectModel {
            options,
            identifier,
            code,
            inputs,
        });
    }
    if let Err(problems) = identifiers.finish() {
        return Ok(Err(problems.iter().map(Diagnostic::of_project).collect()));
    }
    Ok(Ok(Project {
        root,
        sources: source_names(&configuration),
        models,
        configuration,
    }))
}

/// The names of the sources of `configuration`, sorted.
fn source_names(configuration: &Configuration) -> Vec<String> {
    let mut names: Vec<String> = configuration
        .sources
        .iter()
        .map(|source| source.name.clone())
        .collect();
    names.sort();
    names
}

/// The text of the file at `path`, relative to `root`.
fn read_text(
    files: &dyn FileSystem,
    root: &std::path::Path,
    path: &str,
) -> Result<String, CliError> {
    let bytes = files
        .read(&root.join(path))
        .map_err(|error| CliError::Unreadable(error.to_string()))?;
    String::from_utf8(bytes).map_err(|_| CliError::Unreadable(format!("`{path}` is not UTF-8")))
}
