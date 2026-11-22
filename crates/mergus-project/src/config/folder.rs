//! Reading the folder `mergus.toml` files under `models/`: each section names a model of its
//! folder (MG-REQ-0331) and holds the options of the folder schema (MG-REQ-0366).

use std::path::Path;

use mergus_ports::FileSystem;

use super::convert::{self, Converted, Segment};
use super::{ConfigKind, schema, sort_problems, utf8};
use crate::{Layout, LayoutError, ProjectError, closest_name};

/// The name of a folder configuration file.
const FILE: &str = "mergus.toml";

/// The section of a folder file that applies to every model of the folder and below.
pub(crate) const ALL: &str = "all";

/// A valid folder `mergus.toml`.
pub(crate) struct FolderFile {
    /// The folder, relative to the project root, separated by `/`: `models/gold/sales`.
    pub(crate) folder: String,
    /// The path of the file, relative to the project root.
    pub(crate) path: String,
    /// Its JSON and the lines of its keys.
    pub(crate) converted: Converted,
}

/// Reads every `mergus.toml` of `models/` and its folders, in walking order. The models of
/// `layout` tell which sections are valid. Problems are pushed to `problems`, and the files
/// that have some are left out.
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when the port fails and [`LayoutError::Schema`] when the
/// bundled folder schema is broken.
pub(crate) fn read_folders(
    files: &dyn FileSystem,
    root: &Path,
    layout: &Layout,
    problems: &mut Vec<ProjectError>,
) -> Result<Vec<FolderFile>, LayoutError> {
    let mut read = Vec::new();
    let models = root.join("models");
    if files.exists(&models)? {
        let mut walk = Walk {
            files,
            root,
            layout,
            read: &mut read,
            problems,
        };
        walk.folder(&models, "models")?;
    }
    Ok(read)
}

struct Walk<'a> {
    files: &'a dyn FileSystem,
    root: &'a Path,
    layout: &'a Layout,
    read: &'a mut Vec<FolderFile>,
    problems: &'a mut Vec<ProjectError>,
}

impl Walk<'_> {
    /// Reads the `mergus.toml` of the folder at `directory`, named `folder` from the root,
    /// then walks its subfolders. A directory is an entry the port can list.
    fn folder(&mut self, directory: &Path, folder: &str) -> Result<(), LayoutError> {
        let mut subfolders = Vec::new();
        for entry in self.files.list_dir(directory)? {
            let name = entry
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if self.files.list_dir(&entry).is_ok() {
                subfolders.push((entry, name));
            } else if name == FILE {
                self.file(folder)?;
            }
        }
        for (entry, name) in subfolders {
            self.folder(&entry, &format!("{folder}/{name}"))?;
        }
        Ok(())
    }

    /// Reads and checks the `mergus.toml` of `folder`.
    fn file(&mut self, folder: &str) -> Result<(), LayoutError> {
        let path = format!("{folder}/{FILE}");
        let bytes = self.files.read(&self.root.join(&path))?;
        let mut found = Vec::new();
        let converted = match utf8(&path, &bytes, &mut found) {
            Some(text) => convert::convert(&path, text, &mut found),
            None => None,
        };
        if let Some(mut converted) = converted {
            let models = self.models_of(folder);
            check_sections(&path, &mut converted, &models, &mut found);
            schema::validate(ConfigKind::Folder, &path, &converted, &mut found)?;
            if found.is_empty() {
                self.read.push(FolderFile {
                    folder: folder.to_owned(),
                    path,
                    converted,
                });
            }
        }
        sort_problems(&mut found);
        self.problems.append(&mut found);
        Ok(())
    }

    /// The names of the models whose file is directly in `folder`.
    fn models_of(&self, folder: &str) -> Vec<&str> {
        self.layout
            .models
            .iter()
            .filter(|model| parent(&model.path) == folder)
            .map(|model| model.name.as_str())
            .collect()
    }
}

/// Reports every top-level key other than `[all]` that names no model of `models`, with the
/// closest model name, and removes it so that the schema sees only real sections
/// (MG-REQ-0331).
fn check_sections(
    path: &str,
    converted: &mut Converted,
    models: &[&str],
    problems: &mut Vec<ProjectError>,
) {
    let Some(sections) = converted.value.as_object_mut() else {
        return;
    };
    let unknown: Vec<String> = sections
        .keys()
        .filter(|section| *section != ALL && !models.contains(&section.as_str()))
        .cloned()
        .collect();
    for section in unknown {
        sections.remove(&section);
        let line = converted
            .lines
            .get(&[Segment::Key(section.clone())][..])
            .map_or(1, |lines| lines.key);
        problems.push(ProjectError::UnknownSection {
            path: path.to_owned(),
            line,
            suggestion: closest_name(&section, models.iter().copied()).map(str::to_owned),
            section,
        });
    }
}

/// The folder of a path relative to the root: everything before its last `/`.
pub(crate) fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}
