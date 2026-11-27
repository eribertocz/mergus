//! MG-REQ-0636: until server mode exists, a Polars step runs as MG-REQ-0617.1 says: no secret
//! value in its environment, its arguments or the files of its working directory, and a
//! fresh working directory for each step and each run.

use std::path::Path;

use mergus_engine_polars::{Parameter, plan_step, step_dir};
use mergus_ports::real::{RealEnvironment, RealFileSystem};
use mergus_ports::{Environment, FileSystem};
use mergus_sim::SimEnvironment;

use super::{floats, python_with, read, scratch, step, write};

const MODEL: &str = r#"import json, os, sys

def spy(source):
    before = sorted(os.listdir("."))
    seen = {"env": dict(os.environ), "cwd": os.getcwd(), "argv": sys.argv, "before": before}
    with open("seen.json", "w") as file:
        json.dump(seen, file)
    open("leftover", "w").close()
    return source

def other(source):
    return spy(source)
"#;

const SECRET: &str = "s3cr3t-value-0636";

/// The variables the child may see (MG-REQ-0006).
const ALLOWED: [&str; 11] = [
    "PATH",
    "LANG",
    "LC_ALL",
    "TZ",
    "SYSTEMROOT",
    "VIRTUAL_ENV",
    "PYTHONHASHSEED",
    "PYTHONNOUSERSITE",
    "TMP",
    "TEMP",
    "TMPDIR",
];

/// The variables the child sets itself once it runs: Python under the C locale (PEP 538), and
/// `polars` when it is imported, for its allocator.
const SET_BY_THE_CHILD: [&str; 2] = ["LC_CTYPE", "_RJEM_MALLOC_CONF"];

/// What the step saw, read from its working directory.
#[cfg(test)]
fn seen(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&read(&dir.join("seen.json"))).unwrap()
}

/// Fails when a file of `dir` holds the secret.
#[cfg(test)]
fn no_secret_in_files(dir: &Path) {
    for file in RealFileSystem.list_dir(dir).unwrap() {
        let bytes = RealFileSystem.read(&file).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains(SECRET),
            "{} holds the secret",
            file.display()
        );
    }
}

#[test]
fn mg_req_0636_server_mode_isolates_python() {
    let root = scratch("isolation");
    write(&root.join("models").join("model.py"), MODEL);
    let mut vars: Vec<(String, String)> = ["PATH", "VIRTUAL_ENV", "LANG"]
        .iter()
        .filter_map(|name| Some(((*name).to_owned(), RealEnvironment.var(name)?)))
        .collect();
    vars.push(("MERGUS_SECRET_WAREHOUSE".to_owned(), SECRET.to_owned()));
    vars.push(("AWS_SECRET_ACCESS_KEY".to_owned(), SECRET.to_owned()));
    vars.push(("PGPASSWORD".to_owned(), SECRET.to_owned()));
    let environment = SimEnvironment::new(vars, None);
    let first_run = root.join("run-1");
    let Some(python) = python_with(&root, &first_run, &environment) else {
        return;
    };
    let parameters = [Parameter {
        name: "source".to_owned(),
        schema: floats(&["amount"]),
    }];
    let spy = step(&root, "sales", "spy");
    let other = step(&root, "sales", "other");

    plan_step(&python, &RealFileSystem, &spy, &parameters, &first_run).unwrap();
    let dir = step_dir(&first_run, &spy);
    assert!(dir.starts_with(&first_run));
    let first = seen(&dir);
    // The environment is the allowlisted one, and holds no secret value.
    for (name, value) in first["env"].as_object().unwrap() {
        assert!(
            ALLOWED.contains(&name.as_str()) || SET_BY_THE_CHILD.contains(&name.as_str()),
            "{name} reached Python"
        );
        assert!(
            !value.as_str().unwrap().contains(SECRET),
            "{name} holds the secret"
        );
    }
    assert_eq!(first["env"]["TMPDIR"], first_run.display().to_string());
    // The arguments and the files of the working directory hold none either.
    assert!(!first["argv"].to_string().contains(SECRET));
    no_secret_in_files(&dir);
    // The step ran in its own directory under the run's work directory, which was empty.
    assert_eq!(first["cwd"], dir.display().to_string());
    assert!(
        first["before"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file != "leftover")
    );

    // Called again in the same run, the step starts from a fresh directory.
    plan_step(&python, &RealFileSystem, &spy, &parameters, &first_run).unwrap();
    let again = seen(&dir);
    assert!(
        again["before"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file != "leftover")
    );

    // Another step has its own directory.
    plan_step(&python, &RealFileSystem, &other, &parameters, &first_run).unwrap();
    let other_dir = step_dir(&first_run, &other);
    assert_ne!(other_dir, dir);
    assert_eq!(seen(&other_dir)["cwd"], other_dir.display().to_string());

    // Another run has its own directory too.
    let second_run = root.join("run-2");
    plan_step(&python, &RealFileSystem, &spy, &parameters, &second_run).unwrap();
    let second_dir = step_dir(&second_run, &spy);
    assert_ne!(second_dir, dir);
    assert_eq!(seen(&second_dir)["cwd"], second_dir.display().to_string());
    no_secret_in_files(&second_dir);
}
