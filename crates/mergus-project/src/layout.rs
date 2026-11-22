//! Walking the project folders into a [`Layout`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mergus_ports::{FileSystem, PortError};

use crate::error::{LayoutError, NameKind, ProjectError};
use crate::names::{is_valid_name, suggest_valid_name};

/// The language of a model file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModelLanguage {
    /// A `.sql` file.
    Sql,
    /// A `.py` file.
    Python,
}

/// Whether a model is published (MG-REQ-0304).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Publication {
    /// Published as `gold.<schema>.<table>`.
    Gold {
        /// The schema: the folder under `models/gold/`.
        schema: String,
        /// The table: the model name.
        table: String,
    },
    /// Not published.
    Intermediate,
}

/// One model: one `.sql` or `.py` file under `models/` (MG-REQ-0302).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    /// The file stem.
    pub name: String,
    /// The path of the file, relative to the project root, separated by `/`.
    pub path: String,
    /// The language of the file.
    pub language: ModelLanguage,
    /// Whether and where it is published.
    pub publication: Publication,
}

impl Model {
    /// Returns the published name `gold.<schema>.<table>`, or `None` for an intermediate
    /// model.
    #[must_use]
    pub fn published_name(&self) -> Option<String> {
        match &self.publication {
            Publication::Gold { schema, table } => Some(format!("gold.{schema}.{table}")),
            Publication::Intermediate => None,
        }
    }
}

/// The files of a project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// The models, in walking order: the entries of each folder sorted by name, each folder
    /// walked where it sorts.
    pub models: Vec<Model>,
}

/// Reads the layout of the project at `root` through `files`. A missing `models/`,
/// `sources/` or `seeds/` folder is empty. File contents are never read (MG-REQ-0301).
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when the port fails, and with [`LayoutError::Invalid`]
/// carrying every problem found: invalid names in walking order (`models/`, `sources/`,
/// `seeds/`), then duplicate models.
pub fn read_layout(files: &dyn FileSystem, root: &Path) -> Result<Layout, LayoutError> {
    let mut walk = Walk {
        files,
        root,
        models: Vec::new(),
        problems: Vec::new(),
    };
    let models = root.join("models");
    if files.exists(&models)? {
        walk.folder(files.list_dir(&models)?, &mut Vec::new())?;
    }
    walk.stems("sources", "toml", NameKind::Source)?;
    walk.stems("seeds", "csv", NameKind::Seed)?;
    walk.duplicates();
    if walk.problems.is_empty() {
        Ok(Layout {
            models: walk.models,
        })
    } else {
        Err(LayoutError::Invalid(walk.problems))
    }
}

/// The state of one walk of a project.
struct Walk<'a> {
    files: &'a dyn FileSystem,
    root: &'a Path,
    models: Vec<Model>,
    problems: Vec<ProjectError>,
}

impl Walk<'_> {
    /// Walks the `entries` of the folder `models/<folders>`. An entry is a directory when the
    /// port can list it, and a file otherwise: the port has no other way to tell them apart.
    fn folder(
        &mut self,
        entries: Vec<PathBuf>,
        folders: &mut Vec<String>,
    ) -> Result<(), PortError> {
        for entry in entries {
            let name = file_name(&entry);
            if let Ok(children) = self.files.list_dir(&entry) {
                let kind = if folders.len() == 1 && folders[0] == "gold" {
                    NameKind::Schema
                } else {
                    NameKind::Folder
                };
                self.check_name(kind, &name, &entry);
                folders.push(name);
                self.folder(children, folders)?;
                folders.pop();
            } else if let Some(language) = model_language(&entry) {
                self.model(&entry, language, folders);
            }
        }
        Ok(())
    }

    /// Records the model file at `path`, in the folder `models/<folders>`.
    fn model(&mut self, path: &Path, language: ModelLanguage, folders: &[String]) {
        let name = stem(path);
        self.check_name(NameKind::Model, &name, path);
        let publication = match folders {
            [gold, schema] if gold == "gold" => Publication::Gold {
                schema: schema.clone(),
                table: name.clone(),
            },
            _ => Publication::Intermediate,
        };
        self.models.push(Model {
            name,
            path: self.relative(path),
            language,
            publication,
        });
    }

    /// Checks the stems of the `*.<extension>` files directly in the folder `folder`.
    fn stems(&mut self, folder: &str, extension: &str, kind: NameKind) -> Result<(), PortError> {
        let folder = self.root.join(folder);
        if !self.files.exists(&folder)? {
            return Ok(());
        }
        for entry in self.files.list_dir(&folder)? {
            if entry.extension().is_some_and(|found| found == extension)
                && self.files.list_dir(&entry).is_err()
            {
                self.check_name(kind, &stem(&entry), &entry);
            }
        }
        Ok(())
    }

    /// Records an error when `name`, carried by `path`, is not valid (MG-REQ-0305).
    fn check_name(&mut self, kind: NameKind, name: &str, path: &Path) {
        if !is_valid_name(name) {
            self.problems.push(ProjectError::InvalidName {
                kind,
                name: name.to_owned(),
                path: self.relative(path),
                suggestion: suggest_valid_name(name),
            });
        }
    }

    /// Records an error for every model whose name an earlier model has (MG-REQ-0303).
    fn duplicates(&mut self) {
        let mut first: BTreeMap<&str, &str> = BTreeMap::new();
        for model in &self.models {
            if let Some(earlier) = first.get(model.name.as_str()) {
                self.problems.push(ProjectError::DuplicateModel {
                    name: model.name.clone(),
                    first: (*earlier).to_owned(),
                    second: model.path.clone(),
                });
            } else {
                first.insert(&model.name, &model.path);
            }
        }
    }

    /// Returns `path` relative to the project root, its components separated by `/`.
    fn relative(&self, path: &Path) -> String {
        let relative = path.strip_prefix(self.root).unwrap_or(path);
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// Returns the language of a model file, from its extension.
fn model_language(path: &Path) -> Option<ModelLanguage> {
    match path.extension()?.to_str()? {
        "sql" => Some(ModelLanguage::Sql),
        "py" => Some(ModelLanguage::Python),
        _ => None,
    }
}

/// Returns the last component of `path` as text.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Returns the stem of `path` as text.
fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
