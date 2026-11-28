//! Stages 1 to 4 read only project files, find every error at once and report cycles.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mergus_ports::real::RealSqlParser;
use mergus_ports::{FileSystem, PortError, SeekRead};
use mergus_project::ProjectError;
use mergus_runtime::{ReadError, ReadPorts, read_project};
use mergus_sim::SimPython;

use crate::fixture::{COUNT, at, cleared, orders, plan, project, read, root};

/// A file system that records every path it is asked about.
#[cfg(test)]
struct Watched<'a> {
    inner: &'a dyn FileSystem,
    reads: Mutex<Vec<PathBuf>>,
    writes: Mutex<Vec<PathBuf>>,
}

#[cfg(test)]
impl Watched<'_> {
    fn read_at(&self, path: &Path) {
        self.reads.lock().unwrap().push(path.to_path_buf());
    }

    fn wrote_at(&self, path: &Path) {
        self.writes.lock().unwrap().push(path.to_path_buf());
    }
}

#[cfg(test)]
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
        .create_dir_all(&at(".mergus/work/candidates/f0"))
        .unwrap();
    files
        .write(&at(".mergus/work/candidates/f0/result.arrow"), b"rows")
        .unwrap();
    files.create_dir_all(&at("lake/data")).unwrap();
    files
        .write(&at("lake/data/orders.parquet"), b"rows")
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

#[cfg(test)]
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
        ("models/Bad-Name.sql", "SELECT 1 AS id"),
        ("sources/erp.toml", "unknown = 1\n"),
    ]);
    // The layout is read before the configuration, but `project.toml` and `sources/` come
    // before `models/` in it.
    let groups = errors_of(&files);
    let paths: Vec<&str> = groups.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(
        paths,
        ["project.toml", "sources/erp.toml", "models/Bad-Name.sql"]
    );
    let lines: Vec<Option<usize>> = groups[0].1.iter().map(ProjectError::line).collect();
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
fn mg_req_0702_identifier_errors_join_the_others() {
    // `ids.toml` gives both models one identifier, and one model reads a missing table. Two
    // models can only collide by name (MG-0312), which the layout already refuses (MG-0301),
    // so the collision shows as the entry of `ids.toml` (MG-0311).
    let ids = "[model]\na_orders = \"mdl_sw4a23b2ujmhopsw\"\nb_orders = \"mdl_sw4a23b2ujmhopsw\"\n";
    let files = project(&[
        ("project.toml", "name = \"sales\"\n"),
        ("ids.toml", ids),
        ("models/a_orders.sql", "SELECT 1 AS id"),
        ("models/b_orders.sql", "SELECT * FROM missing_one"),
    ]);
    let codes: Vec<&str> = errors_of(&files)
        .iter()
        .flat_map(|(_, errors)| errors.iter().map(ProjectError::code))
        .collect();
    assert!(codes.contains(&"MG-0311"), "{codes:?}");
    assert!(codes.contains(&"MG-0313"), "{codes:?}");
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

/// The text and the tree the real `Python` port gives of `models/b.py`, which reads `a`.
const B_PY: &str = "def b(a):\n    return \"SELECT * FROM a\"\n";
const B_TREE: &str = r#"{"calls":[],"functions":[{"async":false,"column":1,"line":1,"name":"b","parameters":[{"column":7,"kind":"positional","line":1,"name":"a"}],"returns":[{"column":5,"kind":"string","line":2,"value":"SELECT * FROM a"}]}],"imports":[],"version":1}"#;

#[test]
fn mg_req_0703_cycle_through_python_reported_in_order() {
    let files = project(&[
        ("project.toml", "name = \"sales\"\n"),
        ("models/a.sql", "SELECT *\nFROM b"),
        ("models/b.py", B_PY),
    ]);
    let python = SimPython::new();
    python.record(B_PY, B_TREE);
    let parser = RealSqlParser::new();
    let ports = ReadPorts {
        files: &files,
        parser: &parser,
        python: Some(&python),
        database: None,
    };
    let Err(ReadError::Invalid(groups)) = read_project(&ports, root()) else {
        panic!("the cycle is not reported");
    };
    let errors: Vec<&ProjectError> = groups.iter().flat_map(|group| &group.errors).collect();
    let expected = ProjectError::ModelCycle {
        models: vec!["a".to_owned(), "b".to_owned()],
        references: vec![
            ("models/a.sql".to_owned(), 2),
            ("models/b.py".to_owned(), 1),
        ],
    };
    assert_eq!(errors, [&expected]);
}

/// The text and the tree the real `Python` port gives of `models/b.py`, whose parameter `a`
/// sits on line 3, below its `def`.
const B_PY_LONG: &str = "def b(\n    *,\n    a,\n):\n    return \"SELECT * FROM a\"\n";
const B_TREE_LONG: &str = r#"{"calls":[],"functions":[{"async":false,"column":1,"line":1,"name":"b","parameters":[{"column":5,"kind":"keyword_only","line":3,"name":"a"}],"returns":[{"column":5,"kind":"string","line":5,"value":"SELECT * FROM a"}]}],"imports":[],"version":1}"#;

#[test]
fn mg_req_0375_cycle_lines_exact_when_read() {
    // A comment and a string literal name `b` before the real reference on line 3.
    let files = project(&[
        ("project.toml", "name = \"sales\"\n"),
        ("models/a.sql", "-- reads b\nSELECT 'b' AS note, *\nFROM b"),
        ("models/b.py", B_PY_LONG),
    ]);
    let python = SimPython::new();
    python.record(B_PY_LONG, B_TREE_LONG);
    let parser = RealSqlParser::new();
    let ports = ReadPorts {
        files: &files,
        parser: &parser,
        python: Some(&python),
        database: None,
    };
    let Err(ReadError::Invalid(groups)) = read_project(&ports, root()) else {
        panic!("the cycle is not reported");
    };
    let errors: Vec<&ProjectError> = groups.iter().flat_map(|group| &group.errors).collect();
    let expected = ProjectError::ModelCycle {
        models: vec!["a".to_owned(), "b".to_owned()],
        references: vec![
            ("models/a.sql".to_owned(), 3),
            ("models/b.py".to_owned(), 3),
        ],
    };
    assert_eq!(errors, [&expected]);
}
