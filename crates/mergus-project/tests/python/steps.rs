//! Public functions are steps (MG-REQ-0612); the one named like the file is the result
//! (MG-REQ-0615).

use mergus_project::{InputSource, ProjectError, PythonModelError, StepEngine};

use super::{problems, read, summary};

#[test]
fn mg_req_0612_public_functions_are_steps() {
    let model = read("sales_by_segment").unwrap();
    assert_eq!(model.path, "models/sales_by_segment.py");
    let names: Vec<(&str, usize)> = model
        .steps
        .iter()
        .map(|step| (step.name.as_str(), step.line))
        .collect();
    // `_rate` and the function nested in it are private; `Helper.method` is a method.
    assert_eq!(
        names,
        [("joined", 4), ("sales_by_segment", 17), ("in_usd", 21)]
    );

    // A public `async def` is no step and is rejected; a private one is a helper.
    let found = problems("async_step");
    assert_eq!(
        summary(&found),
        [(
            "MG-0618",
            Some(1),
            "`models/async_step.py` line 1: step `fetched` is an `async def`; a step is a plain \
             `def`, or rename it `_fetched` to make it a private helper"
                .to_owned()
        )]
    );
    let error = ProjectError::Python(found[0].clone());
    assert_eq!(error.code(), "MG-0618");
    assert_eq!(error.path(), "models/async_step.py");
    assert_eq!(error.line(), Some(1));
}

#[test]
fn mg_req_0615_result_function_required() {
    // The result is the step named like the file, wherever it is written.
    let model = read("sales_by_segment").unwrap();
    assert_eq!(model.name, "sales_by_segment");
    let result = model.result().unwrap();
    assert_eq!(result.name, "sales_by_segment");
    assert_eq!(result.line, 17);
    assert_eq!(
        result.inputs[0].source,
        InputSource::Step("in_usd".to_owned())
    );
    assert!(matches!(result.engine, StepEngine::DuckDb { .. }));

    // A private `_nameless` is not the result.
    let found = problems("nameless");
    assert_eq!(
        found,
        [PythonModelError::MissingResult {
            path: "models/nameless.py".to_owned(),
            stem: "nameless".to_owned(),
        }]
    );
    assert_eq!(
        summary(&found),
        [(
            "MG-0623",
            None,
            "`models/nameless.py` has no function `nameless`; define `def nameless(...)` for the \
             model's result"
                .to_owned()
        )]
    );
}
