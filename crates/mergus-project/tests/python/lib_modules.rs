//! Shared code lives in `lib` (MG-REQ-0618).

use std::collections::BTreeMap;
use std::path::Path;

use mergus_contracts::hash::sha256_hex;
use mergus_ports::FileSystem;
use mergus_project::{LayoutError, PythonModel, read_python_lib, read_python_model};
use mergus_sim::SimFileSystem;

use super::{MODELS, fixture, known, python};

/// The root of the project of this test.
const ROOT: &str = "/project";

/// Every `lib` fixture.
const LIB: &[(&str, &str, &str)] = &[
    fixture!("lib/helpers"),
    fixture!("lib/rates"),
    fixture!("lib/rates_changed"),
    fixture!("lib/unused"),
];

/// The text of the `lib` fixture `name`.
fn text(name: &str) -> &'static str {
    let found = LIB.iter().find(|(fixture, _, _)| *fixture == name);
    found.map_or("", |(_, text, _)| *text)
}

/// Reads the model fixture `name` in a project whose `lib/` holds `modules`, each a module
/// name and the `lib` fixture it holds.
fn read(name: &str, modules: &[(&str, &str)]) -> Result<PythonModel, LayoutError> {
    let fs = SimFileSystem::new();
    fs.create_dir_all(&Path::new(ROOT).join("lib/cache.py"))?;
    for (module, fixture) in modules {
        let path = Path::new(ROOT).join(format!("lib/{module}.py"));
        fs.write(&path, text(fixture).as_bytes())?;
    }
    fs.write(&Path::new(ROOT).join("lib/notes.txt"), b"import rates")?;
    let all: Vec<(&str, &str, &str)> = MODELS.iter().chain(LIB).copied().collect();
    let python = python(&all);
    let lib = read_python_lib(&fs, Path::new(ROOT), &python)?;
    let found = MODELS.iter().find(|(fixture, _, _)| *fixture == name);
    let source = found.map_or("", |(_, text, _)| *text);
    let path = format!("models/{name}.py");
    read_python_model(&path, source.as_bytes(), &known(), &lib, &python)
}

fn hash(fixture: &str) -> String {
    format!("sha256:{}", sha256_hex(text(fixture).as_bytes()))
}

#[test]
fn mg_req_0618_lib_changes_refingerprint_importers() {
    let modules = [
        ("helpers", "lib/helpers"),
        ("rates", "lib/rates"),
        ("unused", "lib/unused"),
    ];
    // `uses_lib` imports `helpers`, which imports `rates`; `os` and `polars` are not in
    // `lib`, `from . import` is relative, and `unused` is imported by no model.
    let model = read("uses_lib", &modules).unwrap();
    let expected = BTreeMap::from([
        ("lib:helpers".to_owned(), hash("lib/helpers")),
        ("lib:rates".to_owned(), hash("lib/rates")),
    ]);
    assert_eq!(model.lib_inputs, expected);
    assert!(read("plain", &modules).unwrap().lib_inputs.is_empty());

    // Changing `rates` changes the inputs of `uses_lib` only.
    let changed = [
        ("helpers", "lib/helpers"),
        ("rates", "lib/rates_changed"),
        ("unused", "lib/unused"),
    ];
    let model = read("uses_lib", &changed).unwrap();
    assert_eq!(model.lib_inputs["lib:rates"], hash("lib/rates_changed"));
    assert_eq!(model.lib_inputs["lib:helpers"], hash("lib/helpers"));
    assert!(read("plain", &changed).unwrap().lib_inputs.is_empty());

    // A project without `lib/` has no module.
    let lib = read_python_lib(&SimFileSystem::new(), Path::new(ROOT), &python(&[])).unwrap();
    assert!(lib.modules.is_empty());
}
