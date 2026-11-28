//! The local file system.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

/// How many names [`unique_sibling`] tries before it gives up.
const SIBLING_ATTEMPTS: u32 = 64;

/// Creates a new file beside `path`, under a name no other call of this process or of another
/// one uses, and returns its path and the open file.
fn unique_sibling(path: &Path) -> Result<(PathBuf, File), PortError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut last = None;
    for _ in 0..SIBLING_ATTEMPTS {
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let sibling = path.with_file_name(format!(".{name}.{}.{number}.tmp", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&sibling)
        {
            Ok(file) => return Ok((sibling, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => last = Some(error),
            Err(error) => return Err(port_error(&shown(path), &error)),
        }
    }
    let error = last.unwrap_or_else(|| std::io::Error::other("no attempt was made"));
    Err(port_error(&shown(path), &error))
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

    fn create_new(&self, path: &Path, contents: &[u8]) -> Result<bool, PortError> {
        // Written whole under a name of its own, then linked into place: the link fails if
        // anything is at `path`, and a reader never sees the file half written.
        let (temporary, mut file) = unique_sibling(path)?;
        let written = file
            .write_all(contents)
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::hard_link(&temporary, path));
        drop(file);
        let removed = fs::remove_file(&temporary);
        match written {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
            Err(error) => return Err(port_error(&shown(path), &error)),
        }
        removed.map_err(|error| port_error(&shown(&temporary), &error))?;
        #[cfg(unix)]
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| port_error(&shown(parent), &error))?;
        }
        Ok(true)
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        fs::remove_file(path).map_err(|error| port_error(&shown(path), &error))
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), PortError> {
        fs::remove_dir_all(path).map_err(|error| port_error(&shown(path), &error))
    }

    /// Flushes the file at `from` to the disk first, so that after a power loss `to` holds
    /// either the old entry or every byte of the new one; on Unix it then flushes the
    /// directory of `to`, so the rename itself is on the disk.
    fn rename(&self, from: &Path, to: &Path) -> Result<(), PortError> {
        let failed = |error: std::io::Error| port_error(&shown(from), &error);
        // Windows flushes only a file opened for writing; nothing is written or truncated.
        let source = fs::OpenOptions::new().read(true).write(true).open(from);
        source.and_then(|file| file.sync_all()).map_err(failed)?;
        fs::rename(from, to).map_err(failed)?;
        #[cfg(unix)]
        if let Some(parent) = to.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| port_error(&shown(parent), &error))?;
        }
        Ok(())
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

    #[test]
    fn real_file_system_renames_in_one_step() {
        let dir = scratch("rename");
        let fs = RealFileSystem;
        fs.write(&dir.join("complete.json.tmp"), b"new").unwrap();
        fs.write(&dir.join("complete.json"), b"old").unwrap();
        fs.rename(&dir.join("complete.json.tmp"), &dir.join("complete.json"))
            .unwrap();
        assert_eq!(fs.read(&dir.join("complete.json")).unwrap(), b"new");
        assert!(!fs.exists(&dir.join("complete.json.tmp")).unwrap());
        assert!(matches!(
            fs.rename(&dir.join("missing"), &dir.join("other")),
            Err(PortError::NotFound(_))
        ));
    }

    #[test]
    fn real_file_system_creates_a_new_file_exclusively() {
        let dir = scratch("create-new");
        let fs = RealFileSystem;
        let path = dir.join("claim.json");
        assert!(fs.create_new(&path, b"first").unwrap());
        assert!(!fs.create_new(&path, b"second").unwrap());
        assert_eq!(fs.read(&path).unwrap(), b"first");
        assert!(matches!(
            fs.create_new(&dir.join("no").join("parent"), b""),
            Err(PortError::NotFound(_))
        ));
        // Nothing but the file is left beside it.
        assert_eq!(fs.list_dir(&dir).unwrap(), [path]);
    }
}
