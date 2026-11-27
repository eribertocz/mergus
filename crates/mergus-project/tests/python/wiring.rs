//! Parameter names wire the steps (MG-REQ-0614).

use mergus_project::{InputSource, PythonModelError, StepInput};

use super::{problems, read, summary};

fn input(parameter: &str, source: InputSource) -> StepInput {
    StepInput {
        parameter: parameter.to_owned(),
        source,
    }
}

#[test]
fn mg_req_0614_parameters_resolve_inputs() {
    let model = read("wiring").unwrap();
    let step = |name: &str| {
        let found = model.steps.iter().find(|step| step.name == name);
        found.map(|step| step.inputs.clone()).unwrap_or_default()
    };
    // A step never reads itself: the parameter `stg_orders` of step `stg_orders` is the model.
    assert_eq!(
        step("stg_orders"),
        [input(
            "stg_orders",
            InputSource::Model("stg_orders".to_owned())
        )]
    );
    // A step of the file comes before the model of the same name; raw tables and seeds are
    // named without their schema; every kind of parameter but `*` and `**` is an input.
    assert_eq!(
        step("wiring"),
        [
            input("stg_orders", InputSource::Step("stg_orders".to_owned())),
            input("erp__orders", InputSource::Raw("erp__orders".to_owned())),
            input(
                "seed__countries",
                InputSource::Raw("seed__countries".to_owned())
            ),
            input("fact_sales", InputSource::Model("fact_sales".to_owned())),
        ]
    );
    assert_miswired();
}

/// Checks the problems of the fixture `miswired` (MG-REQ-0614.1, MG-REQ-0614.2).
fn assert_miswired() {
    let found = problems("miswired");
    let unresolved = |step: &str, parameter: &str, at: (usize, usize), hint: Option<&str>| {
        PythonModelError::UnresolvedParameter {
            path: "models/miswired.py".to_owned(),
            step: step.to_owned(),
            parameter: parameter.to_owned(),
            line: at.0,
            column: at.1,
            suggestion: hint.map(str::to_owned),
        }
    };
    let variadic = |parameter: &str, column: usize| PythonModelError::VariadicParameter {
        path: "models/miswired.py".to_owned(),
        step: "miswired".to_owned(),
        parameter: parameter.to_owned(),
        line: 1,
        column,
    };
    assert_eq!(
        found,
        [
            unresolved("miswired", "stg_ordrs", (1, 14), Some("stg_orders")),
            // More than 3 edits from every known name: no suggestion.
            unresolved("miswired", "zzzzzzzzzz", (1, 25), None),
            // `abc` and `abd` are both 1 edit away: the first in name order wins.
            unresolved("miswired", "abx", (1, 37), Some("abc")),
            variadic("*rest", 43),
            variadic("**options", 51),
            // The step itself and the model of this file are not candidates.
            unresolved("miswired_too", "miswired_too", (5, 18), None),
            PythonModelError::StepCycle {
                path: "models/miswired.py".to_owned(),
                steps: vec!["ping".to_owned(), "pong".to_owned()],
                line: 9,
            },
        ]
    );
    assert_miswired_messages(&found);
}

/// Checks the codes and messages of the problems of the fixture `miswired`.
fn assert_miswired_messages(found: &[PythonModelError]) {
    let codes: Vec<&str> = found.iter().map(PythonModelError::code).collect();
    assert_eq!(
        codes,
        [
            "MG-0620", "MG-0620", "MG-0620", "MG-0621", "MG-0621", "MG-0620", "MG-0622"
        ]
    );
    let messages = summary(found);
    assert_eq!(
        messages[0].2,
        "`models/miswired.py` line 1 column 14: parameter `stg_ordrs` of step `miswired` is not \
         a step of this file, a model, a raw table or a seed; did you mean `stg_orders`?"
    );
    assert_eq!(
        messages[3].2,
        "`models/miswired.py` line 1 column 43: parameter `*rest` of step `miswired` takes any \
         number of inputs; name each input as its own parameter"
    );
    assert_eq!(
        messages[6].2,
        "`models/miswired.py` line 9: steps `ping` -> `pong` -> `ping` form a cycle"
    );
}
