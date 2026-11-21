//! A simulated file system held in memory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mergus_ports::{FileSystem, PortError};

use crate::rng::lock;

#[derive(Clone, Debug)]
enum Node {
    File(Vec<u8>),
    Directory,
}

/// Files and directories in memory. Paths are compared by component (MG-REQ-0189); a path
/// with no parent, such as `/` or a single name, always has an existing parent.
#[derive(Debug, Default)]
pub struct SimFileSystem {
    nodes: Mutex<BTreeMap<PathBuf, Node>>,
}

impl SimFileSystem {
    /// Returns an empty file system.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Returns the path as text for an error message.
fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// Returns whether `path` is an existing directory, or has no parent to need.
fn is_directory(nodes: &BTreeMap<PathBuf, Node>, path: Option<&Path>) -> bool {
    match path {
        None => true,
        Some(path) if path.as_os_str().is_empty() => true,
        Some(path) => matches!(nodes.get(path), Some(Node::Directory)),
    }
}

impl FileSystem for SimFileSystem {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError> {
        match lock(&self.nodes).get(path) {
            Some(Node::File(contents)) => Ok(contents.clone()),
            Some(Node::Directory) => Err(PortError::Failed(format!(
                "`{}` is a directory",
                shown(path)
            ))),
            None => Err(PortError::NotFound(shown(path))),
        }
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError> {
        let mut nodes = lock(&self.nodes);
        if !is_directory(&nodes, path.parent()) {
            return Err(PortError::NotFound(shown(path)));
        }
        if matches!(nodes.get(path), Some(Node::Directory)) {
            return Err(PortError::Failed(format!(
                "`{}` is a directory",
                shown(path)
            )));
        }
        nodes.insert(path.to_path_buf(), Node::File(contents.to_vec()));
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        let mut nodes = lock(&self.nodes);
        match nodes.get(path) {
            Some(Node::File(_)) => {
                nodes.remove(path);
                Ok(())
            }
            Some(Node::Directory) => Err(PortError::Failed(format!(
                "`{}` is a directory",
                shown(path)
            ))),
            None => Err(PortError::NotFound(shown(path))),
        }
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), PortError> {
        let mut nodes = lock(&self.nodes);
        let mut current = PathBuf::new();
        for component in path.components() {
            current.push(component);
            if current.parent().is_none() {
                continue;
            }
            match nodes.get(&current) {
                Some(Node::File(_)) => {
                    return Err(PortError::Failed(format!(
                        "`{}` is a file",
                        shown(&current)
                    )));
                }
                Some(Node::Directory) => {}
                None => {
                    nodes.insert(current.clone(), Node::Directory);
                }
            }
        }
        Ok(())
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, PortError> {
        let nodes = lock(&self.nodes);
        if !is_directory(&nodes, Some(path)) && path.parent().is_some() {
            return Err(PortError::NotFound(shown(path)));
        }
        Ok(nodes
            .keys()
            .filter(|key| key.parent() == Some(path))
            .cloned()
            .collect())
    }

    fn exists(&self, path: &Path) -> Result<bool, PortError> {
        Ok(path.parent().is_none() || lock(&self.nodes).contains_key(path))
    }
}
