//! Tests of the `subprocess` level, with the real Python child and `polars` (MG-REQ-0617,
//! MG-REQ-0619, MG-REQ-0636). They skip when no interpreter with `polars` is found and `CI`
//! is unset; in CI they fail instead.

mod isolation;
mod plain;
mod planning;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{Array, Float64Array, RecordBatch};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::ipc::reader::FileReader;
use mergus_engine_polars::{Step, child_environment};
use mergus_ports::real::{RealEnvironment, RealFileSystem, RealPython, RealRandom};
use mergus_ports::{Environment, FileSystem, Python, Random};

/// A new, empty directory for the test `name`, under the target directory of the tests.
pub fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "mergus-polars-{name}-{:016x}",
        RealRandom::new().next_u64()
    ));
    RealFileSystem.create_dir_all(&dir).unwrap();
    dir
}

/// Writes `contents` at `path`, creating its parents.
pub fn write(path: &Path, contents: &str) {
    RealFileSystem
        .create_dir_all(path.parent().unwrap())
        .unwrap();
    RealFileSystem.write(path, contents.as_bytes()).unwrap();
}

/// The interpreter of a project at `root`, with the child environment of a run whose work
/// directory is `work_dir` and the variables of `environment`. `None` when it has no
/// `polars` and `CI` is unset.
pub fn python_with(
    root: &Path,
    work_dir: &Path,
    environment: &dyn Environment,
) -> Option<RealPython> {
    let env = child_environment(&work_dir.display().to_string(), environment);
    let found = RealPython::locate(None, root, env);
    let ci = RealEnvironment.var("CI").is_some();
    let python = match found {
        Ok(python) => python,
        Err(_) if !ci => return None,
        Err(error) => panic!("no Python interpreter in CI: {error}"),
    };
    let check = python
        .run("import site\nsite.main()\nimport polars\n", &[], root)
        .unwrap();
    match check.code {
        Some(0) => Some(python),
        _ if !ci => None,
        _ => panic!(
            "the Python of CI cannot import polars: {}",
            String::from_utf8_lossy(&check.stderr)
        ),
    }
}

/// [`python_with`] the environment of this process.
pub fn python(root: &Path, work_dir: &Path) -> Option<RealPython> {
    python_with(root, work_dir, &RealEnvironment)
}

/// The step `name` of the model `model` in the file `models/model.py` of `root`.
pub fn step(root: &Path, model: &str, name: &str) -> Step {
    Step {
        model: model.to_owned(),
        step: name.to_owned(),
        module: root.join("models").join("model.py"),
        lib: root.join("lib"),
    }
}

/// A schema of nullable `Float64` columns.
pub fn floats(names: &[&str]) -> SchemaRef {
    Arc::new(Schema::new(
        names
            .iter()
            .map(|name| Field::new(*name, DataType::Float64, true))
            .collect::<Vec<_>>(),
    ))
}

/// A batch of `schema` with one `Float64` column per slice.
pub fn batch(schema: &SchemaRef, columns: &[&[f64]]) -> RecordBatch {
    let arrays: Vec<Arc<dyn Array>> = columns
        .iter()
        .map(|values| Arc::new(Float64Array::from(values.to_vec())) as Arc<dyn Array>)
        .collect();
    RecordBatch::try_new(schema.clone(), arrays).unwrap()
}

/// The values of the `Float64` column `name` of `batches`.
pub fn column(batches: &[RecordBatch], name: &str) -> Vec<f64> {
    batches
        .iter()
        .flat_map(|batch| {
            let array = batch.column_by_name(name).unwrap();
            let floats = array.as_any().downcast_ref::<Float64Array>().unwrap();
            floats.values().to_vec()
        })
        .collect()
}

/// The batches of the Arrow IPC file at `path`.
pub fn read_ipc(path: &Path) -> Vec<RecordBatch> {
    let reader = FileReader::try_new(RealFileSystem.open(path).unwrap(), None).unwrap();
    reader.map(Result::unwrap).collect()
}

/// The text of the file at `path`.
pub fn read(path: &Path) -> String {
    String::from_utf8(RealFileSystem.read(path).unwrap()).unwrap()
}

/// Runs `script` in `dir` and fails the test when it fails.
pub fn run_script(python: &RealPython, script: &str, dir: &Path) {
    let output = python.run(script, &[], dir).unwrap();
    let mut stderr = std::io::stderr();
    stderr.write_all(&output.stderr).unwrap();
    assert_eq!(output.code, Some(0), "the script failed");
}
