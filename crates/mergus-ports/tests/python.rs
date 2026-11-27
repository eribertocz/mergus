//! Tests of the real `Python` port (MG-REQ-0611): it reads a file's tree without importing or
//! executing it, and gives the tree recorded in `tests/fixtures/python/`, which the simulated
//! port replays. A missing interpreter skips them, except in CI (`CI` set), where it fails.

use std::path::{Path, PathBuf};

use mergus_ports::real::{RealEnvironment, RealFileSystem, RealPython, RealRandom};
use mergus_ports::{
    Environment, FileSystem, PortError, Python, PythonParameterKind, PythonReturned, PythonSegment,
    PythonTree, Random,
};

const MODEL: &str = include_str!("fixtures/python/model.py");
const MODEL_TREE: &str = include_str!("fixtures/python/model.json");

/// A new, empty directory for the test `name`, under the target directory of the tests.
#[cfg(test)]
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "mergus-python-{name}-{:016x}",
        RealRandom::new().next_u64()
    ));
    RealFileSystem.create_dir_all(&dir).unwrap();
    dir
}

/// The search path of this process, the only variable the interpreter is given here.
#[cfg(test)]
fn search_path() -> Vec<(String, String)> {
    RealEnvironment
        .var("PATH")
        .map(|path| ("PATH".to_owned(), path))
        .into_iter()
        .collect()
}

/// The interpreter on the search path for a project at `root`, or `None` when there is none
/// and `CI` is unset.
#[cfg(test)]
fn python(root: &Path) -> Option<RealPython> {
    match RealPython::locate(None, root, search_path()) {
        Ok(python) => Some(python),
        Err(PortError::PythonNotFound { .. }) if RealEnvironment.var("CI").is_none() => None,
        Err(error) => panic!("no Python interpreter in CI: {error}"),
    }
}

#[test]
fn mg_req_0611_python_parsed_not_imported() {
    let root = scratch("not-imported");
    let imported = root.join("imported");
    let executed = root.join("executed");
    // A module next to the file, which leaves a mark if it is ever imported.
    RealFileSystem
        .write(
            &root.join("sidecar.py"),
            format!("open({:?}, 'w').close()\n", imported.display().to_string()).as_bytes(),
        )
        .unwrap();
    let source = format!(
        "import sidecar\nimport mergus_module_that_does_not_exist\nfrom sidecar import nothing\n\
         open({:?}, 'w').close()\n\ndef model(upstream):\n    return \"SELECT 1\"\n",
        executed.display().to_string()
    );
    let Some(python) = python(&root) else {
        return;
    };
    let tree = python.parse(&source).unwrap();
    assert!(
        !RealFileSystem.exists(&imported).unwrap(),
        "sidecar was imported"
    );
    assert!(
        !RealFileSystem.exists(&executed).unwrap(),
        "the file was executed"
    );
    let modules: Vec<_> = tree.imports.iter().map(|i| i.module.as_deref()).collect();
    assert_eq!(
        modules,
        [
            Some("sidecar"),
            Some("mergus_module_that_does_not_exist"),
            Some("sidecar")
        ]
    );
    assert_eq!(tree.functions.len(), 1);
    assert_eq!(tree.functions[0].name, "model");
    assert_eq!(tree.functions[0].parameters[0].name, "upstream");
    assert_eq!(
        tree.functions[0].returns[0].value,
        PythonReturned::String {
            value: "SELECT 1".to_owned()
        }
    );
}

#[test]
fn mg_req_0611_real_tree_is_the_recorded_one() {
    let root = scratch("recorded");
    let Some(python) = python(&root) else {
        return;
    };
    let expected = PythonTree::from_json(MODEL_TREE).unwrap();
    assert_eq!(python.parse(MODEL).unwrap(), expected);
}

#[test]
fn mg_req_0611_tree_shape_is_frozen() {
    // Every field of the document is read, and reading then writing it changes nothing.
    let tree = PythonTree::from_json(MODEL_TREE).unwrap();
    let written = serde_json::to_value(&tree).unwrap();
    let mut recorded: serde_json::Value = serde_json::from_str(MODEL_TREE).unwrap();
    recorded.as_object_mut().unwrap().remove("version");
    assert_eq!(written, recorded);
}

#[test]
fn mg_req_0611_tree_functions_and_parameters() {
    let tree = PythonTree::from_json(MODEL_TREE).unwrap();
    let names: Vec<_> = tree.functions.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["joined", "in_usd", "latest"]);
    let joined = &tree.functions[0];
    assert_eq!((joined.line, joined.column, joined.is_async), (9, 1, false));
    assert!(matches!(
        &joined.returns[0].value,
        PythonReturned::String { value } if value.contains("JOIN exchange_rates r USING (date)")
    ));
    let in_usd = &tree.functions[1];
    let kinds: Vec<_> = in_usd.parameters.iter().map(|p| p.kind).collect();
    assert_eq!(
        kinds,
        [
            PythonParameterKind::PositionalOnly,
            PythonParameterKind::KeywordOnly,
            PythonParameterKind::VarKeyword
        ]
    );
}

#[test]
fn mg_req_0611_tree_returns() {
    let tree = PythonTree::from_json(MODEL_TREE).unwrap();
    let in_usd = &tree.functions[1];
    // The `return 0` of `nested` is not one of `in_usd`.
    let returned: Vec<_> = in_usd.returns.iter().map(|r| r.value.clone()).collect();
    assert_eq!(
        returned,
        [
            PythonReturned::None,
            PythonReturned::Expression {
                node: "Call".to_owned()
            }
        ]
    );
    assert!(tree.functions[2].is_async);
    assert_eq!(
        tree.functions[2].returns[0].value,
        PythonReturned::Expression {
            node: "JoinedStr".to_owned()
        }
    );
}

#[test]
fn mg_req_0611_tree_imports_and_calls() {
    let tree = PythonTree::from_json(MODEL_TREE).unwrap();
    let relative = &tree.imports[1];
    assert_eq!(
        (
            relative.module.as_deref(),
            relative.name.as_deref(),
            relative.alias.as_deref(),
            relative.level
        ),
        (Some("helpers"), Some("clean"), Some("tidy"), 1)
    );
    let unique = tree
        .calls
        .iter()
        .find(|c| c.name.as_deref() == Some("unique"))
        .unwrap();
    assert_eq!(unique.keywords, [Some("subset".to_owned()), None]);
    assert_eq!(
        unique.receiver[..3],
        [
            PythonSegment::Name {
                name: "joined".to_owned()
            },
            PythonSegment::Attribute {
                name: "sort".to_owned()
            },
            PythonSegment::Call,
        ]
    );
}

#[test]
fn mg_req_0611_syntax_error_is_typed() {
    let root = scratch("syntax");
    let Some(python) = python(&root) else {
        return;
    };
    let error = python.parse("def model(:\n    return 1\n").unwrap_err();
    assert!(
        matches!(
            error,
            PortError::PythonSyntax {
                line: Some(1),
                column: Some(_),
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn mg_req_0611_long_chain_is_not_a_syntax_error() {
    let root = scratch("long-chain");
    let Some(python) = python(&root) else {
        return;
    };
    // A valid chain of 1,000 links is read without recursion.
    let source = format!("x = df{}.first()\n", ".b".repeat(999));
    let tree = python.parse(&source).unwrap();
    assert_eq!(tree.calls.len(), 1);
    assert_eq!(tree.calls[0].name.as_deref(), Some("first"));
    assert_eq!(tree.calls[0].receiver.len(), 1_000);
    assert_eq!(
        tree.calls[0].receiver[0],
        PythonSegment::Name {
            name: "df".to_owned()
        }
    );
}

#[test]
fn python_columns_count_characters() {
    let root = scratch("columns");
    let Some(python) = python(&root) else {
        return;
    };
    let tree = python.parse("é = f(1)\n").unwrap();
    assert_eq!((tree.calls[0].line, tree.calls[0].column), (1, 5));
}

#[test]
fn python_interpreter_not_found_lists_what_was_tried() {
    let root = scratch("not-found");
    let error = RealPython::locate(None, &root, Vec::new()).unwrap_err();
    let PortError::PythonNotFound { searched } = error else {
        panic!("{error:?}");
    };
    assert_eq!(searched.len(), 1);
    assert!(searched[0].contains(".venv"), "{searched:?}");

    let missing = Path::new("bin/missing-python");
    let error = RealPython::locate(Some(missing), &root, search_path()).unwrap_err();
    assert_eq!(
        error,
        PortError::PythonNotFound {
            searched: vec![root.join(missing).display().to_string()]
        }
    );
}

#[test]
fn mg_req_0617_python_runs_a_program_in_its_directory() {
    let root = scratch("run");
    let step = root.join("step");
    RealFileSystem.create_dir_all(&step).unwrap();
    let mut env = search_path();
    env.push(("MERGUS_GIVEN".to_owned(), "1".to_owned()));
    let python = match RealPython::locate(None, &root, env) {
        Ok(python) => python,
        Err(PortError::PythonNotFound { .. }) if RealEnvironment.var("CI").is_none() => return,
        Err(error) => panic!("no Python interpreter in CI: {error}"),
    };
    // Python itself sets `LC_CTYPE` when the locale is C (PEP 538), and macOS adds
    // `__CF_USER_TEXT_ENCODING`: both are left out.
    let script = "import json, os, sys\n\
        env = {k: v for k, v in os.environ.items() \
               if k not in ('LC_CTYPE', '__CF_USER_TEXT_ENCODING')}\n\
        open('mark', 'w').close()\n\
        print(json.dumps([sys.argv[1:], os.path.basename(os.getcwd()), sorted(env)]))\n\
        sys.stderr.write('done')\n\
        sys.exit(3)\n";
    let args = vec!["a b".to_owned(), "--c".to_owned()];
    let output = python.run(script, &args, &step).unwrap();
    assert_eq!(output.code, Some(3));
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "done");
    let seen: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        seen,
        serde_json::json!([["a b", "--c"], "step", ["MERGUS_GIVEN", "PATH"]])
    );
    // The program ran in `step`, where it left its mark.
    assert!(RealFileSystem.exists(&step.join("mark")).unwrap());
    assert!(matches!(
        python.run(script, &args, &root.join("missing")),
        Err(PortError::NotFound(_))
    ));
}
