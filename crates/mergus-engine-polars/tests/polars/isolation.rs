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

/// Fails when the environment, the arguments or the files of the step `dir` hold the secret,
/// or when the environment has a variable outside the allowlist.
#[cfg(test)]
fn no_secret_reaches_python(first: &serde_json::Value, run: &Path, dir: &Path) {
    for (name, value) in first["env"].as_object().unwrap() {
        let allowed = ALLOWED.contains(&name.as_str()) || SET_BY_THE_CHILD.contains(&name.as_str());
        assert!(allowed, "{name} reached Python");
        assert!(
            !value.as_str().unwrap().contains(SECRET),
            "{name} holds the secret"
        );
    }
    assert_eq!(first["env"]["TMPDIR"], run.display().to_string());
    assert!(!first["argv"].to_string().contains(SECRET));
    no_secret_in_files(dir);
}

/// The search path and locale of this process, and secret values in variables that hold
/// credentials.
#[cfg(test)]
fn with_secrets() -> SimEnvironment {
    let mut vars: Vec<(String, String)> = ["PATH", "VIRTUAL_ENV", "LANG"]
        .iter()
        .filter_map(|name| Some(((*name).to_owned(), RealEnvironment.var(name)?)))
        .collect();
    for name in ["WAREHOUSE_TOKEN", "AWS_SECRET_ACCESS_KEY", "PGPASSWORD"] {
        vars.push((name.to_owned(), SECRET.to_owned()));
    }
    SimEnvironment::new(vars, None)
}

/// Whether the step's directory held nothing an earlier call left when the step ran.
#[cfg(test)]
fn was_fresh(seen: &serde_json::Value) -> bool {
    let before = seen["before"].as_array().unwrap();
    before.iter().all(|file| file != "leftover")
}

#[test]
fn mg_req_0636_server_mode_isolates_python() {
    let root = scratch("isolation");
    write(&root.join("models").join("model.py"), MODEL);
    let environment = with_secrets();
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
    no_secret_reaches_python(&first, &first_run, &dir);
    // The step ran in its own directory under the run's work directory, which was empty.
    assert_eq!(first["cwd"], dir.display().to_string());
    assert!(was_fresh(&first));

    // Called again in the same run, the step starts from a fresh directory.
    plan_step(&python, &RealFileSystem, &spy, &parameters, &first_run).unwrap();
    assert!(was_fresh(&seen(&dir)));

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
