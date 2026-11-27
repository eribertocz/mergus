//! Shared code lives in `lib` (MG-REQ-0618): its modules, read without importing them, and
//! the fingerprint inputs of the models that import them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mergus_contracts::hash::sha256_hex;
use mergus_ports::{FileSystem, Python, PythonImport};

use crate::LayoutError;

/// The folder of the shared modules.
const LIB: &str = "lib";

/// The prefix of the fingerprint input of a `lib` module (MG-REQ-0618.2).
const INPUT_PREFIX: &str = "lib:";

/// One `lib/<module>.py`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonLibModule {
    /// The path of the file, relative to the project root.
    pub path: String,
    /// `sha256:<hex>`, the SHA-256 of the bytes of the file (MG-REQ-0618.2).
    pub hash: String,
    /// The other `lib` modules it imports directly, by name (MG-REQ-0618.1).
    pub imports: BTreeSet<String>,
}

/// The modules of `lib/`, by module name: the stem of the file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PythonLib {
    /// Every module, by name.
    pub modules: BTreeMap<String, PythonLibModule>,
}

impl PythonLib {
    /// Returns the fingerprint inputs of a file with `imports`: `lib:<module>` to the hash of
    /// each `lib` module it imports, directly or through other `lib` modules
    /// (MG-REQ-0618.2).
    #[must_use]
    pub fn fingerprint_inputs(&self, imports: &[PythonImport]) -> BTreeMap<String, String> {
        let mut inputs = BTreeMap::new();
        let mut pending: Vec<String> = self.imported(imports).into_iter().collect();
        while let Some(name) = pending.pop() {
            let Some(module) = self.modules.get(&name) else {
                continue;
            };
            if inputs
                .insert(format!("{INPUT_PREFIX}{name}"), module.hash.clone())
                .is_none()
            {
                pending.extend(module.imports.iter().cloned());
            }
        }
        inputs
    }

    /// The `lib` modules `imports` import directly: each absolute import whose first name
    /// part is a module of `lib` (MG-REQ-0618.1).
    fn imported(&self, imports: &[PythonImport]) -> BTreeSet<String> {
        imports
            .iter()
            .filter(|import| import.level == 0)
            .filter_map(|import| import.module.as_deref())
            .filter_map(|module| module.split('.').next())
            .filter(|first| self.modules.contains_key(*first))
            .map(str::to_owned)
            .collect()
    }
}

/// Reads every `lib/*.py` of the project at `root` through `python`, which parses each
/// without importing it (MG-REQ-0618.1). A project without `lib/` has no module.
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when a file cannot be read or `python` fails, including
/// when a module is not valid Python.
pub fn read_python_lib(
    files: &dyn FileSystem,
    root: &Path,
    python: &dyn Python,
) -> Result<PythonLib, LayoutError> {
    let folder = root.join(LIB);
    if !files.exists(&folder)? {
        return Ok(PythonLib::default());
    }
    let mut trees = Vec::new();
    for entry in files.list_dir(&folder)? {
        let is_py = entry.extension().is_some_and(|found| found == "py");
        if !is_py || files.list_dir(&entry).is_ok() {
            continue;
        }
        let Some(name) = entry.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let bytes = files.read(&entry)?;
        let tree = python.parse(&String::from_utf8_lossy(&bytes))?;
        let module = PythonLibModule {
            path: format!("{LIB}/{name}.py"),
            hash: format!("sha256:{}", sha256_hex(&bytes)),
            imports: BTreeSet::new(),
        };
        trees.push((name.to_owned(), module, tree.imports));
    }
    let mut lib = PythonLib {
        modules: trees
            .iter()
            .map(|(name, module, _)| (name.clone(), module.clone()))
            .collect(),
    };
    for (name, _, imports) in &trees {
        let imported = lib.imported(imports);
        if let Some(module) = lib.modules.get_mut(name) {
            module.imports = imported;
        }
    }
    Ok(lib)
}
