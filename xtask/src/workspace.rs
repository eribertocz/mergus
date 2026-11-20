//! Locate the workspace and its members.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::failure::Failure;

/// One member of the workspace: a directory with a `Cargo.toml`.
pub(crate) struct Member {
    /// The directory name, which is also the crate name (MG-REQ-0110).
    pub(crate) name: String,
    /// The directory that holds the member's `Cargo.toml`.
    pub(crate) dir: PathBuf,
}

/// Returns the root of the workspace: the parent of the `xtask` directory.
///
/// # Errors
///
/// Fails if the `xtask` directory has no parent.
pub(crate) fn root() -> Result<PathBuf, Failure> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| Failure::new("the xtask directory has no parent"))
}

/// Lists the members of the workspace, `members = ["crates/*", "xtask"]`, sorted by name.
///
/// # Errors
///
/// Fails if `crates/` exists but cannot be read.
pub(crate) fn members(root: &Path) -> Result<Vec<Member>, Failure> {
    let mut members = Vec::new();
    let crates = root.join("crates");
    if crates.is_dir() {
        for dir in sorted_entries(&crates)? {
            if dir.join("Cargo.toml").is_file() {
                members.push(member(dir));
            }
        }
    }
    members.push(member(root.join("xtask")));
    Ok(members)
}

/// Lists every `.rs` file under `dir`, skipping `target` directories, sorted by path.
///
/// # Errors
///
/// Fails if a directory cannot be read.
pub(crate) fn rust_files(dir: &Path) -> Result<Vec<PathBuf>, Failure> {
    let mut files = Vec::new();
    for path in sorted_entries(dir)? {
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                files.extend(rust_files(&path)?);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(files)
}

/// Reads a UTF-8 text file.
///
/// # Errors
///
/// Fails if the file cannot be read or is not UTF-8.
pub(crate) fn read(path: &Path) -> Result<String, Failure> {
    fs::read_to_string(path).map_err(|error| Failure::io(path, &error))
}

/// Lists the entries of a directory, sorted by path.
///
/// # Errors
///
/// Fails if the directory cannot be read.
pub(crate) fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>, Failure> {
    let entries = fs::read_dir(dir).map_err(|error| Failure::io(dir, &error))?;
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| Failure::io(dir, &error))?;
    paths.sort();
    Ok(paths)
}

/// Shows `path` relative to `root` when it is inside it, with `/` between its parts on
/// every system, so that findings read the same on Linux, macOS and Windows.
pub(crate) fn show(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let mut text = String::new();
    for component in relative.components() {
        match component {
            Component::RootDir => text.push('/'),
            Component::Prefix(_) => text.push_str(&component.as_os_str().to_string_lossy()),
            _ => {
                if !text.is_empty() && !text.ends_with('/') {
                    text.push('/');
                }
                text.push_str(&component.as_os_str().to_string_lossy());
            }
        }
    }
    text
}

fn member(dir: PathBuf) -> Member {
    let name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Member { name, dir }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;
    use std::path::PathBuf;

    use std::path::Path;

    use super::{members, rust_files, show};

    /// Creates an empty temporary directory unique to `name`.
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mergus-xtask-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn members_are_crates_with_a_manifest_then_xtask() {
        let root = scratch("members");
        for name in ["b", "a", "no-manifest"] {
            fs::create_dir_all(root.join("crates").join(name)).unwrap();
        }
        fs::write(root.join("crates/a/Cargo.toml"), "").unwrap();
        fs::write(root.join("crates/b/Cargo.toml"), "").unwrap();
        let names: Vec<_> = members(&root)
            .unwrap()
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["a", "b", "xtask"]);
    }

    #[test]
    fn rust_files_skip_target_directories() {
        let root = scratch("rust-files");
        fs::create_dir_all(root.join("src/inner")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        for file in [
            "src/lib.rs",
            "src/inner/mod.rs",
            "src/notes.md",
            "target/gen.rs",
        ] {
            fs::write(root.join(file), "").unwrap();
        }
        let files = rust_files(&root).unwrap();
        assert_eq!(
            files,
            [root.join("src/inner/mod.rs"), root.join("src/lib.rs")]
        );
    }

    #[test]
    fn mg_req_0126_paths_are_shown_relative_with_slashes() {
        let root = Path::new("w");
        let nested = root.join("other").join("mergus-catalog");
        assert_eq!(show(root, &nested), "other/mergus-catalog");
        let outside = Path::new("elsewhere").join("crates").join("a");
        assert_eq!(show(root, &outside), "elsewhere/crates/a");
        assert_eq!(show(root, root), "");
    }
}
