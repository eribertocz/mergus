//! The local file system.

use std::fs;
use std::path::{Path, PathBuf};

use super::port_error;
use crate::{FileSystem, PortError};

/// The operating system's file system.
#[derive(Clone, Copy, Debug, Default)]
pub struct RealFileSystem;

/// Returns the path as text for an error message.
fn shown(path: &Path) -> String {
    path.display().to_string()
}

impl FileSystem for RealFileSystem {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError> {
        fs::read(path).map_err(|error| port_error(&shown(path), &error))
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError> {
        fs::write(path, contents).map_err(|error| port_error(&shown(path), &error))
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        fs::remove_file(path).map_err(|error| port_error(&shown(path), &error))
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), PortError> {
        fs::create_dir_all(path).map_err(|error| port_error(&shown(path), &error))
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, PortError> {
        let error = |error: std::io::Error| port_error(&shown(path), &error);
        let mut entries = Vec::new();
        for entry in fs::read_dir(path).map_err(error)? {
            entries.push(entry.map_err(error)?.path());
        }
        entries.sort();
        Ok(entries)
    }

    fn exists(&self, path: &Path) -> Result<bool, PortError> {
        path.try_exists()
            .map_err(|error| port_error(&shown(path), &error))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::RealFileSystem;
    use crate::{FileSystem, PortError};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("mergus-ports-tests")
            .join(format!("{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        RealFileSystem.create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn real_file_system_writes_reads_lists_and_removes() {
        let dir = scratch("file-system");
        let fs = RealFileSystem;
        fs.write(&dir.join("b.txt"), b"two").unwrap();
        fs.write(&dir.join("a.txt"), b"one").unwrap();
        assert_eq!(fs.read(&dir.join("a.txt")).unwrap(), b"one");
        assert_eq!(
            fs.list_dir(&dir).unwrap(),
            [dir.join("a.txt"), dir.join("b.txt")]
        );
        fs.remove_file(&dir.join("a.txt")).unwrap();
        assert!(!fs.exists(&dir.join("a.txt")).unwrap());
        assert!(matches!(
            fs.read(&dir.join("a.txt")),
            Err(PortError::NotFound(_))
        ));
    }
}
