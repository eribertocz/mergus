//! Stages 1 to 4 read only project files, find every error at once and report cycles.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mergus_ports::real::RealSqlParser;
use mergus_ports::{FileSystem, PortError, SeekRead};
use mergus_project::ProjectError;
use mergus_runtime::{ReadError, ReadPorts, read_project};

use crate::fixture::{COUNT, cleared, orders, plan, project, read, root};

/// A file system that records every path it is asked about.
struct Watched<'a> {
    inner: &'a dyn FileSystem,
    reads: Mutex<Vec<PathBuf>>,
    writes: Mutex<Vec<PathBuf>>,
}

impl Watched<'_> {
    fn read_at(&self, path: &Path) {
        self.reads.lock().unwrap().push(path.to_path_buf());
    }

    fn wrote_at(&self, path: &Path) {
        self.writes.lock().unwrap().push(path.to_path_buf());
    }
}

impl FileSystem for Watched<'_> {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError> {
        self.read_at(path);
        self.inner.read(path)
    }

    fn open(&self, path: &Path) -> Result<Box<dyn SeekRead>, PortError> {
        self.read_at(path);
        self.inner.open(path)
    }

    fn create(&self, path: &Path) -> Result<Box<dyn Write + Send>, PortError> {
        self.wrote_at(path);
        self.inner.create(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError> {
        self.wrote_at(path);
        self.inner.write(path, contents)
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        self.wrote_at(path);
        self.inner.remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), PortError> {
        self.wrote_at(path);
        self.inner.remove_dir_all(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), PortError> {
        self.wrote_at(from);
        self.inner.rename(from, to)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), PortError> {
        self.wrote_at(path);
        self.inner.create_dir_all(path)
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, PortError> {
        self.read_at(path);
        self.inner.list_dir(path)
    }

    fn exists(&self, path: &Path) -> Result<bool, PortError> {
        self.inner.exists(path)
    }
}

#[test]
fn mg_req_0701_stages_one_to_four_touch_no_data() {
    let files = orders(COUNT);
    files
        .create_dir_all(&root().join(".mergus/work/candidates/f0"))
        .unwrap();
    files
        .write(
            &root().join(".mergus/work/candidates/f0/result.arrow"),
            b"rows",
        )
        .unwrap();
    files.create_dir_all(&root().join("lake/data")).unwrap();
    files
        .write(&root().join("lake/data/orders.parquet"), b"rows")
        .unwrap();
    let watched = Watched {
        inner: &files,
        reads: Mutex::new(Vec::new()),
        writes: Mutex::new(Vec::new()),
    };
    let project = read(&watched);
    let planned = plan(&project, &BTreeSet::new(), 4);
    let _cleared = cleared();
    assert_eq!(planned.document().steps.len(), 3);
    assert!(watched.writes.lock().unwrap().is_empty());
    let reads = watched.reads.lock().unwrap();
    assert!(!reads.is_empty());
    for path in reads.iter() {
        let relative = path.strip_prefix(root()).unwrap();
        assert!(
            !relative.starts_with(".mergus") && !relative.starts_with("lake"),
            "stages 1 to 4 read `{}`",
            relative.display()
        );
    }
}

fn errors_of(files: &dyn FileSystem) -> Vec<(String, Vec<ProjectError>)> {
    let parser = RealSqlParser::new();
    let ports = ReadPorts {
        files,
        parser: &parser,
        python: None,
        database: None,
    };
    match read_project(&ports, root()) {
        Err(ReadError::Invalid(groups)) => groups
            .into_iter()
            .map(|group| (group.path, group.errors))
            .collect(),
        other => panic!("expected errors, got {other:?}"),
    }
}

#[test]
fn mg_req_0702_all_errors_reported_together() {
    let files = project(&[
        ("project.toml", "name = \"sales\"\nfirst = 1\nsecond = 2\n"),
        ("models/b_orders.sql", "SELECT * FROM missing_one"),
        ("models/a_orders.sql", "SELECT 1 AS id"),
        ("models/mergus.toml", "unknown = 1\n"),
    ]);
    // `project.toml` is read before the folder options, but `models/` walks first.
    let groups = errors_of(&files);
    let paths: Vec<&str> = groups.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, ["models/mergus.toml", "project.toml"]);
    let lines: Vec<Option<usize>> = groups[1].1.iter().map(ProjectError::line).collect();
    assert_eq!(lines, [Some(2), Some(3)]);

    let files = project(&[
        ("project.toml", "name = \"sales\"\n"),
        ("models/b_orders.sql", "SELECT * FROM missing_one"),
        ("models/a_orders.sql", "SELECT 1 AS id"),
        (
            "models/c_orders.sql",
            "SELECT *\nFROM missing_two\nJOIN missing_three ON true",
        ),
    ]);
    let groups = errors_of(&files);
    let paths: Vec<&str> = groups.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, ["models/b_orders.sql", "models/c_orders.sql"]);
    let lines: Vec<Option<usize>> = groups[1].1.iter().map(ProjectError::line).collect();
    assert_eq!(lines, [Some(2), Some(3)]);
}

#[test]
fn mg_req_0703_cycle_reported_in_order() {
    let files = project(&[
        ("project.toml", "name = \"sales\"\n"),
        ("models/a.sql", "SELECT * FROM c"),
        ("models/b.sql", "SELECT * FROM a"),
        ("models/c.sql", "SELECT * FROM b"),
    ]);
    let groups = errors_of(&files);
    let cycles: Vec<&Vec<String>> = groups
        .iter()
        .flat_map(|(_, errors)| errors)
        .filter_map(|error| match error {
            ProjectError::ModelCycle { models, .. } => Some(models),
            _ => None,
        })
        .collect();
    assert_eq!(
        cycles,
        [&vec!["a".to_owned(), "c".to_owned(), "b".to_owned()]]
    );
}
