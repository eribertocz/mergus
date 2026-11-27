//! MG-REQ-0611: the simulated `Python` port replays the trees recorded from the real one in
//! `crates/mergus-ports/tests/fixtures/python/`, and runs nothing.

use mergus_ports::{PortError, Python, PythonTree};
use mergus_sim::SimPython;

const MODEL: &str = include_str!("../../mergus-ports/tests/fixtures/python/model.py");
const MODEL_TREE: &str = include_str!("../../mergus-ports/tests/fixtures/python/model.json");

#[test]
fn mg_req_0611_sim_replays_recorded_trees() {
    let python = SimPython::new();
    python.record(MODEL, MODEL_TREE);
    // The model's imports would fail and it raises at import: its tree is still read.
    let tree = python.parse(MODEL).unwrap();
    assert_eq!(tree, PythonTree::from_json(MODEL_TREE).unwrap());
    let modules: Vec<_> = tree.imports.iter().map(|i| i.module.as_deref()).collect();
    assert_eq!(
        modules,
        [
            Some("polars"),
            Some("helpers"),
            Some("mergus_module_that_does_not_exist")
        ]
    );
    assert_eq!(
        python.parse("x = 1\n"),
        Err(PortError::NotFound(
            "a recorded tree for `x = 1\n`".to_owned()
        ))
    );
}

#[test]
fn mg_req_0611_sim_replays_recorded_syntax_errors() {
    let python = SimPython::new();
    python.record(
        "def f(:\n",
        r#"{"error":{"column":7,"line":1,"message":"invalid syntax"},"version":1}"#,
    );
    assert_eq!(
        python.parse("def f(:\n"),
        Err(PortError::PythonSyntax {
            message: "invalid syntax".to_owned(),
            line: Some(1),
            column: Some(7),
        })
    );
}
