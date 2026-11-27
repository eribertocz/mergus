//! The local file system.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use super::port_error;
use crate::file_system::SeekRead;
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

    fn open(&self, path: &Path) -> Result<Box<dyn SeekRead>, PortError> {
        let file = File::open(path).map_err(|error| port_error(&shown(path), &error))?;
        Ok(Box::new(BufReader::new(file)))
    }

    fn create(&self, path: &Path) -> Result<Box<dyn Write + Send>, PortError> {
        let file = File::create(path).map_err(|error| port_error(&shown(path), &error))?;
        Ok(Box::new(BufWriter::new(file)))
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError> {
        fs::write(path, contents).map_err(|error| port_error(&shown(path), &error))
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        fs::remove_file(path).map_err(|error| port_error(&shown(path), &error))
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), PortError> {
        fs::remove_dir_all(path).map_err(|error| port_error(&shown(path), &error))
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
        fs.create_dir_all(&dir.join("tree").join("leaf")).unwrap();
        fs.write(&dir.join("tree").join("leaf").join("c.txt"), b"three")
            .unwrap();
        fs.remove_dir_all(&dir.join("tree")).unwrap();
        assert!(!fs.exists(&dir.join("tree")).unwrap());
        assert!(matches!(
            fs.remove_dir_all(&dir.join("tree")),
            Err(PortError::NotFound(_))
        ));
        assert!(matches!(
            fs.read(&dir.join("a.txt")),
            Err(PortError::NotFound(_))
        ));
    }

    #[test]
    fn real_file_system_streams_files() {
        use std::io::{Read, Seek, SeekFrom, Write};

        let dir = scratch("streams");
        let fs = RealFileSystem;
        let path = dir.join("stream.bin");
        let mut writer = fs.create(&path).unwrap();
        writer.write_all(b"one ").unwrap();
        writer.write_all(b"two").unwrap();
        writer.flush().unwrap();
        drop(writer);
        let mut reader = fs.open(&path).unwrap();
        reader.seek(SeekFrom::Start(4)).unwrap();
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text, "two");
        assert!(matches!(
            fs.open(&dir.join("missing")),
            Err(PortError::NotFound(_))
        ));
        assert!(matches!(
            fs.create(&dir.join("no").join("parent")),
            Err(PortError::NotFound(_))
        ));
    }
}
