//! MG-REQ-0113: two builds of the same commit produce identical binaries.

use std::ffi::OsString;
use std::fs;
use std::path::Path;

use crate::cargo;
use crate::failure::Failure;
use crate::workspace;

/// Builds the workspace twice from scratch in the same target directory and compares the
/// release artifacts byte for byte.
///
/// # Errors
///
/// Fails if a build fails or if the two builds produced different artifacts.
pub(crate) fn check(root: &Path) -> Result<(), Failure> {
    let base = root.join("target").join("reproducible");
    let build = base.join("build");
    let first = base.join("first");
    remove(&base)?;
    release_build(root, &build)?;
    fs::create_dir_all(&base).map_err(|error| Failure::io(&base, &error))?;
    fs::rename(build.join("release"), &first).map_err(|error| Failure::io(&first, &error))?;
    remove(&build)?;
    release_build(root, &build)?;
    let problems = differences(&first, &build.join("release"))?;
    Failure::from_problems("reproducible-build", &problems)
}

fn release_build(root: &Path, target: &Path) -> Result<(), Failure> {
    let target = OsString::from(target);
    let args = ["build", "--workspace", "--release", "--locked"];
    cargo::run(root, &args, &[("CARGO_TARGET_DIR", &target)])
}

fn remove(dir: &Path) -> Result<(), Failure> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|error| Failure::io(dir, &error))?;
    }
    Ok(())
}

/// Compares the artifacts at the top of two Cargo profile directories and returns one line
/// per difference. Dependency files (`.d`) and Cargo's bookkeeping are not artifacts.
fn differences(first: &Path, second: &Path) -> Result<Vec<String>, Failure> {
    let first_files = artifacts(first)?;
    let second_files = artifacts(second)?;
    let mut problems = Vec::new();
    for name in &first_files {
        if !second_files.contains(name) {
            problems.push(format!("{name}: only in the first build"));
        } else if read_bytes(&first.join(name))? != read_bytes(&second.join(name))? {
            problems.push(format!("{name}: differs between the two builds"));
        }
    }
    for name in second_files
        .iter()
        .filter(|name| !first_files.contains(name))
    {
        problems.push(format!("{name}: only in the second build"));
    }
    Ok(problems)
}

fn artifacts(dir: &Path) -> Result<Vec<String>, Failure> {
    let names = workspace::sorted_entries(dir)?
        .into_iter()
        .filter(|path| path.is_file())
        .filter_map(|path| file_name(&path))
        .filter(|name| !name.ends_with(".d") && !name.starts_with('.'))
        .collect();
    Ok(names)
}

fn file_name(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, Failure> {
    fs::read(path).map_err(|error| Failure::io(path, &error))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::differences;
    use crate::workspace::tests::scratch;

    #[test]
    fn mg_req_0113_identical_builds_pass() {
        let root = scratch("reproducible-same");
        for build in ["a", "b"] {
            fs::create_dir_all(root.join(build)).unwrap();
            fs::write(root.join(build).join("mergus"), [1, 2, 3]).unwrap();
            fs::write(root.join(build).join("mergus.d"), build).unwrap();
            fs::write(root.join(build).join(".cargo-lock"), build).unwrap();
        }
        assert!(
            differences(&root.join("a"), &root.join("b"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn mg_req_0113_different_binaries_are_reported() {
        let root = scratch("reproducible-differ");
        for (build, byte) in [("a", 1), ("b", 2)] {
            fs::create_dir_all(root.join(build)).unwrap();
            fs::write(root.join(build).join("mergus"), [byte]).unwrap();
        }
        fs::write(root.join("a").join("libold.rlib"), []).unwrap();
        fs::write(root.join("b").join("libnew.rlib"), []).unwrap();
        let problems = differences(&root.join("a"), &root.join("b")).unwrap();
        assert_eq!(
            problems,
            [
                "libold.rlib: only in the first build",
                "mergus: differs between the two builds",
                "libnew.rlib: only in the second build",
            ]
        );
    }
}
