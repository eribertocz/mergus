//! The work zone of a run: its scratch directory, deleted when the run ends (MG-REQ-0503).

use std::path::{Path, PathBuf};

use mergus_ports::{FileSystem, PortError};

use super::RunError;

/// The folder of the scratch directories, relative to the project root (MG-REQ-0503.1).
pub const RUNS: &str = ".mergus/work/runs";

/// Deletes every entry of `.mergus/work/runs/` under `root`, left by a run that crashed,
/// then creates the scratch directory of `run_id` and returns it (MG-REQ-0503.1).
pub(crate) fn open_scratch(
    files: &dyn FileSystem,
    root: &Path,
    run_id: &str,
) -> Result<PathBuf, RunError> {
    let runs = root.join(RUNS);
    let failed = |path: &Path, error: PortError| RunError::work(root, path, &error);
    files
        .create_dir_all(&runs)
        .map_err(|error| failed(&runs, error))?;
    for entry in files
        .list_dir(&runs)
        .map_err(|error| failed(&runs, error))?
    {
        remove_entry(files, &entry).map_err(|error| failed(&entry, error))?;
    }
    let scratch = runs.join(run_id);
    files
        .create_dir_all(&scratch)
        .map_err(|error| failed(&scratch, error))?;
    Ok(scratch)
}

/// Deletes the scratch directory `scratch`, if it is still there.
pub(crate) fn remove_scratch(
    files: &dyn FileSystem,
    root: &Path,
    scratch: &Path,
) -> Result<(), RunError> {
    let failed = |error: PortError| RunError::work(root, scratch, &error);
    if files.exists(scratch).map_err(failed)? {
        files.remove_dir_all(scratch).map_err(failed)?;
    }
    Ok(())
}

/// Removes `entry`: a directory with everything in it when the port can list it, a file
/// otherwise, since the port has no other way to tell them apart.
pub(crate) fn remove_entry(files: &dyn FileSystem, entry: &Path) -> Result<(), PortError> {
    if files.list_dir(entry).is_ok() {
        files.remove_dir_all(entry)
    } else {
        files.remove_file(entry)
    }
}
