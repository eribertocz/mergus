//! MG-REQ-0704: `mergus run` executes the Polars steps of a `.py` model, with the real
//! binary, the real Python child and `polars`. The test skips when no interpreter with
//! `polars` is found and `CI` is unset; in CI it fails instead.

use std::path::{Path, PathBuf};
use std::process::Command;

use mergus_ports::real::{RealEnvironment, RealFileSystem, RealPython, RealRandom};
use mergus_ports::{Environment, FileSystem, Python, Random};
use serde_json::Value;

const PROJECT: &str = "name = \"sales\"\n";

const STG: &str = "SELECT 10.0::DOUBLE AS amount UNION ALL SELECT 2.5::DOUBLE AS amount";

/// A `.py` model of two Polars steps: `rated` reads the model `stg_sales`, and `sales_usd`,
/// the model's result, reads the step `rated`. `unused` is read by no step of the result.
const MODEL: &str = r#"import polars as pl


def rated(stg_sales):
    return stg_sales.with_columns(pl.lit(2.0).alias("rate"))


def unused(stg_sales):
    raise RuntimeError("a step the result does not read never runs")


def sales_usd(rated):
    return rated.with_columns((pl.col("amount") * pl.col("rate")).alias("amount_usd"))
"#;

/// A new, empty directory for the test `name`, under the target directory of the tests.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "mergus-cli-{name}-{:016x}",
        RealRandom::new().next_u64()
    ));
    RealFileSystem.create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, contents: &str) {
    RealFileSystem
        .create_dir_all(path.parent().unwrap())
        .unwrap();
    RealFileSystem.write(path, contents.as_bytes()).unwrap();
}

/// Whether the Python the run will find for the project at `root` imports `polars`. `false`
/// skips the test, unless `CI` is set: then it fails.
fn has_polars(root: &Path) -> bool {
    let env = mergus_security::python_command("python3", &[], &root.display().to_string(), &RealEnvironment).env;
    let ci = RealEnvironment.var("CI").is_some();
    let python = match RealPython::locate(None, root, env) {
        Ok(python) => python,
        Err(_) if !ci => return false,
        Err(error) => panic!("no Python interpreter in CI: {error}"),
    };
    let check = python
        .run("import site\nsite.main()\nimport polars\n", &[], root)
        .unwrap();
    match check.code {
        Some(0) => true,
        _ if !ci => false,
        _ => panic!(
            "the Python of CI cannot import polars: {}",
            String::from_utf8_lossy(&check.stderr)
        ),
    }
}

/// The events `mergus run --json` printed in `root`, and its exit code.
fn run(root: &Path) -> (Vec<Value>, Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_mergus"))
        .args(["run", "--json"])
        .current_dir(root)
        .output()
        .unwrap();
    let events = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (events, output.status.code(), stderr)
}

#[test]
fn mg_req_0704_polars_step_executes_in_run() {
    let root = scratch("polars-run");
    write(&root.join("project.toml"), PROJECT);
    write(&root.join("models").join("stg_sales.sql"), STG);
    write(&root.join("models").join("sales_usd.py"), MODEL);
    if !has_polars(&root) {
        return;
    }
    let (events, code, stderr) = run(&root);
    assert_eq!(code, Some(0), "{events:?}\n{stderr}");

    let of = |kind: &str| -> Vec<&Value> {
        events.iter().filter(|event| event["type"] == kind).collect()
    };
    let started = of("step_started");
    let polars: Vec<&&Value> = started
        .iter()
        .filter(|event| event["engine"] == "polars")
        .collect();
    assert_eq!(polars.len(), 1, "{events:?}");
    assert_eq!(polars[0]["model"], "sales_usd");
    let finished = of("step_finished")
        .into_iter()
        .find(|event| event["model"] == "sales_usd")
        .unwrap();
    assert_eq!(finished["step_id"], polars[0]["step_id"]);
    assert_eq!(finished["status"], "succeeded", "{finished}");
    assert_eq!(finished["rows"], 2, "{finished}");

    // The candidate of every step is complete; nothing is left in scratch.
    let candidates = root.join(".mergus").join("work").join("candidates");
    let complete = RealFileSystem
        .list_dir(&candidates)
        .unwrap()
        .iter()
        .filter(|entry| RealFileSystem.exists(&entry.join("complete.json")).unwrap())
        .count();
    assert_eq!(complete, 2);
    let runs = root.join(".mergus").join("work").join("runs");
    assert!(RealFileSystem.list_dir(&runs).unwrap().is_empty());
}
