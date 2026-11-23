//! MG-REQ-1344 and MG-REQ-1345: finding the project and `mergus validate`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mergus_ports::{FileSystem, PortError};
use mergus_sim::{SimFileSystem, SimRandom};

use super::{PROJECT, ROOT, Setup, broken, mergus, project};

fn in_dir(files: &dyn FileSystem, cwd: &str) -> super::Outcome {
    let random = SimRandom::new(7);
    Setup {
        files,
        cwd,
        args: &["validate", "--json"],
        vars: &[],
        random: &random,
        seed: 7,
        terminal: false,
    }
    .run()
}

#[test]
fn mg_req_1344_project_found_upwards() {
    let files = broken();
    files
        .create_dir_all(&Path::new(ROOT).join("models/staging/deep"))
        .unwrap();
    let below = in_dir(&files, &format!("{ROOT}/models/staging/deep"));
    let at_root = in_dir(&files, ROOT);
    assert_eq!(below.exit, 1);
    assert_eq!(below.of("diagnostic"), at_root.of("diagnostic"));

    files
        .write(
            &Path::new(ROOT).join("models/staging/project.toml"),
            PROJECT.as_bytes(),
        )
        .unwrap();
    let nearest = in_dir(&files, &format!("{ROOT}/models/staging/deep"));
    assert_eq!(nearest.exit, 0, "the nearest project wins: {nearest:?}");

    let empty = SimFileSystem::new();
    empty.create_dir_all(Path::new("/tmp/elsewhere")).unwrap();
    let none = in_dir(&empty, "/tmp/elsewhere");
    assert_eq!(none.exit, 1);
    let diagnostic = &none.of("diagnostic")[0];
    assert_eq!(diagnostic["code"], "MG-1302");
    assert!(diagnostic["hint"].as_str().unwrap().contains("mergus init"));
    let human = mergus(&project(&[]), &["validate"]);
    assert_eq!(human.exit, 1);
    assert!(human.stderr.contains("mergus init"), "{}", human.stderr);
}

/// A file system that refuses and records every change.
struct ReadOnly {
    inner: SimFileSystem,
    changes: Mutex<Vec<PathBuf>>,
}

impl ReadOnly {
    fn refuse(&self, path: &Path) -> Result<(), PortError> {
        self.changes.lock().unwrap().push(path.to_path_buf());
        Err(PortError::Failed("read only".to_owned()))
    }
}

impl FileSystem for ReadOnly {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError> {
        self.inner.read(path)
    }

    fn write(&self, path: &Path, _contents: &[u8]) -> Result<(), PortError> {
        self.refuse(path)
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        self.refuse(path)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), PortError> {
        self.refuse(path)
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, PortError> {
        self.inner.list_dir(path)
    }

    fn exists(&self, path: &Path) -> Result<bool, PortError> {
        self.inner.exists(path)
    }
}

#[test]
fn mg_req_1345_validate_reports_all_and_writes_nothing() {
    let files = ReadOnly {
        inner: project(&[
            ("project.toml", "name = \"sales\"\nnmae = 1\n"),
            ("sources/crm.toml", "kind = \n"),
            ("models/Orders.sql", "select 1"),
            ("models/a/dup.sql", "select 1"),
            ("models/b/dup.sql", "select 1"),
        ]),
        changes: Mutex::new(Vec::new()),
    };
    let outcome = in_dir(&files, ROOT);
    let codes: Vec<String> = outcome
        .of("diagnostic")
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        codes,
        ["MG-0302", "MG-0301", "MG-0304", "MG-0303"],
        "{outcome:?}"
    );
    assert_eq!(outcome.exit, 1);
    assert_eq!(*files.changes.lock().unwrap(), Vec::<PathBuf>::new());

    let valid = project(&[("project.toml", PROJECT), ("models/orders.sql", "select 1")]);
    let ok = mergus(&valid, &["validate", "--json"]);
    assert_eq!(ok.exit, 0);
    assert!(ok.of("diagnostic").is_empty());
    assert_eq!(ok.of("run_finished")[0]["status"], "succeeded");
    let human = mergus(&valid, &["validate"]);
    assert_eq!((human.exit, human.stdout.as_str()), (0, ""));
}
