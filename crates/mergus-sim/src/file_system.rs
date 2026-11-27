//! A simulated file system held in memory.

use std::collections::BTreeMap;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mergus_ports::{FileSystem, PortError, SeekRead};

use crate::rng::lock;

#[derive(Clone, Debug)]
enum Node {
    File(Vec<u8>),
    Directory,
}

/// Files and directories in memory. Paths are compared by component (MG-REQ-0189); a path
/// with no parent, such as `/` or a single name, always has an existing parent. A stream from
/// [`FileSystem::open`] reads the file as it was when opened; one from
/// [`FileSystem::create`] stores what it holds each time it is flushed, and when dropped.
#[derive(Debug, Default)]
pub struct SimFileSystem {
    nodes: Arc<Mutex<BTreeMap<PathBuf, Node>>>,
}

/// A stream of [`SimFileSystem::create`]: it keeps the bytes written and stores them as the
/// file on each flush and when dropped.
struct SimWriter {
    nodes: Arc<Mutex<BTreeMap<PathBuf, Node>>>,
    path: PathBuf,
    contents: Vec<u8>,
}

impl SimWriter {
    fn store(&self) {
        lock(&self.nodes).insert(self.path.clone(), Node::File(self.contents.clone()));
    }
}

impl Write for SimWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.contents.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.store();
        Ok(())
    }
}

impl Drop for SimWriter {
    fn drop(&mut self) {
        self.store();
    }
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

    fn open(&self, path: &Path) -> Result<Box<dyn SeekRead>, PortError> {
        Ok(Box::new(Cursor::new(self.read(path)?)))
    }

    fn create(&self, path: &Path) -> Result<Box<dyn Write + Send>, PortError> {
        self.write(path, &[])?;
        Ok(Box::new(SimWriter {
            nodes: Arc::clone(&self.nodes),
            path: path.to_path_buf(),
            contents: Vec::new(),
        }))
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

    fn remove_dir_all(&self, path: &Path) -> Result<(), PortError> {
        let mut nodes = lock(&self.nodes);
        match nodes.get(path) {
            Some(Node::Directory) => {
                nodes.retain(|key, _| !key.starts_with(path));
                Ok(())
            }
            Some(Node::File(_)) => Err(PortError::Failed(format!("`{}` is a file", shown(path)))),
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
