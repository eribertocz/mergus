//! The streaming access of the simulated file system: `open` reads the stored bytes and
//! `create` stores what it was given on flush and on drop.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use mergus_ports::{FileSystem, PortError};
use mergus_sim::SimFileSystem;

#[test]
fn sim_file_system_streams_files() {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/work")).unwrap();
    let path = Path::new("/work/stream.bin");
    let mut writer = files.create(path).unwrap();
    writer.write_all(b"one ").unwrap();
    writer.flush().unwrap();
    assert_eq!(files.read(path).unwrap(), b"one ");
    writer.write_all(b"two").unwrap();
    drop(writer);
    assert_eq!(files.read(path).unwrap(), b"one two");

    let mut reader = files.open(path).unwrap();
    reader.seek(SeekFrom::Start(4)).unwrap();
    let mut text = String::new();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text, "two");
}

#[test]
fn sim_file_system_streams_fail_as_reads_and_writes_do() {
    let files = SimFileSystem::new();
    assert!(matches!(
        files.open(Path::new("/missing")),
        Err(PortError::NotFound(_))
    ));
    assert!(matches!(
        files.create(Path::new("/no/parent")),
        Err(PortError::NotFound(_))
    ));
    files.create_dir_all(Path::new("/work/dir")).unwrap();
    assert!(matches!(
        files.create(Path::new("/work/dir")),
        Err(PortError::Failed(_))
    ));
}

#[test]
fn sim_file_system_removes_a_directory_tree() {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new("/work/tree/leaf")).unwrap();
    files
        .write(Path::new("/work/tree/leaf/c.txt"), b"three")
        .unwrap();
    files
        .write(Path::new("/work/treetop.txt"), b"kept")
        .unwrap();
    files.remove_dir_all(Path::new("/work/tree")).unwrap();
    assert!(!files.exists(Path::new("/work/tree/leaf/c.txt")).unwrap());
    assert!(!files.exists(Path::new("/work/tree")).unwrap());
    assert!(files.exists(Path::new("/work/treetop.txt")).unwrap());
    assert!(matches!(
        files.remove_dir_all(Path::new("/work/tree")),
        Err(PortError::NotFound(_))
    ));
    assert!(matches!(
        files.remove_dir_all(Path::new("/work/treetop.txt")),
        Err(PortError::Failed(_))
    ));
}
