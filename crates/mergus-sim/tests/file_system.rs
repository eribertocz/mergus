//! The rename of the simulated file system: one step, replacing the target, and a crash
//! before or after it on demand.

use std::path::Path;

use mergus_ports::{FileSystem, PortError};
use mergus_sim::SimFileSystem;

#[cfg(test)]
fn with_temporary() -> SimFileSystem {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/c")).unwrap();
    files
        .write(Path::new("/c/complete.json.tmp"), b"new")
        .unwrap();
    files
}

#[test]
fn sim_file_system_renames_in_one_step() {
    let files = with_temporary();
    files.write(Path::new("/c/complete.json"), b"old").unwrap();
    files
        .rename(
            Path::new("/c/complete.json.tmp"),
            Path::new("/c/complete.json"),
        )
        .unwrap();
    assert_eq!(files.read(Path::new("/c/complete.json")).unwrap(), b"new");
    assert!(!files.exists(Path::new("/c/complete.json.tmp")).unwrap());
    assert!(matches!(
        files.rename(Path::new("/c/missing"), Path::new("/c/other")),
        Err(PortError::NotFound(_))
    ));
    assert!(matches!(
        files.rename(Path::new("/c/complete.json"), Path::new("/none/x")),
        Err(PortError::NotFound(_))
    ));
    assert!(matches!(
        files.rename(Path::new("/c"), Path::new("/d")),
        Err(PortError::Failed(_))
    ));
}

#[test]
fn sim_file_system_crashes_before_a_rename() {
    let files = with_temporary();
    files.crash_before_next_rename();
    let renamed = files.rename(
        Path::new("/c/complete.json.tmp"),
        Path::new("/c/complete.json"),
    );
    assert!(matches!(renamed, Err(PortError::Failed(_))));
    assert!(files.exists(Path::new("/c/complete.json.tmp")).unwrap());
    assert!(!files.exists(Path::new("/c/complete.json")).unwrap());
    files
        .rename(
            Path::new("/c/complete.json.tmp"),
            Path::new("/c/complete.json"),
        )
        .unwrap();
}

#[test]
fn sim_file_system_crashes_after_a_rename() {
    let files = with_temporary();
    files.crash_after_next_rename();
    let renamed = files.rename(
        Path::new("/c/complete.json.tmp"),
        Path::new("/c/complete.json"),
    );
    assert!(matches!(renamed, Err(PortError::Failed(_))));
    assert!(!files.exists(Path::new("/c/complete.json.tmp")).unwrap());
    assert_eq!(files.read(Path::new("/c/complete.json")).unwrap(), b"new");
}
