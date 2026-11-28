//! Parameter names wire the steps (MG-REQ-0614).

use mergus_project::{InputSource, PythonModelError, StepInput};

use super::{problems, read, summary};

/// The input of `parameter` on `line`, which reads `source`.
fn input(parameter: &str, line: usize, source: InputSource) -> StepInput {
    StepInput {
        parameter: parameter.to_owned(),
        line,
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
            1,
            InputSource::Model("stg_orders".to_owned())
        )]
    );
    // A step of the file comes before the model of the same name; raw tables and seeds are
    // named without their schema; keyword-only parameters and those with a default value are
    // inputs.
    assert_eq!(
        step("wiring"),
        [
            input("stg_orders", 5, InputSource::Step("stg_orders".to_owned())),
            input("erp__orders", 5, InputSource::Raw("erp__orders".to_owned())),
            input(
                "seed__countries",
                5,
                InputSource::Raw("seed__countries".to_owned())
            ),
            input("fact_sales", 5, InputSource::Model("fact_sales".to_owned())),
            input("abc", 5, InputSource::Model("abc".to_owned())),
        ]
    );
    assert_miswired();
}

/// The problem of the variadic `parameter` of step `miswired` at `column` of line 1.
fn variadic(parameter: &str, column: usize) -> PythonModelError {
    PythonModelError::VariadicParameter {
        path: "models/miswired.py".to_owned(),
        step: "miswired".to_owned(),
        parameter: parameter.to_owned(),
        line: 1,
        column,
    }
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
    let cycle = |steps: &[&str], line: usize| PythonModelError::StepCycle {
        path: "models/miswired.py".to_owned(),
        steps: steps.iter().map(|step| (*step).to_owned()).collect(),
        line,
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
            // Mergus passes inputs by keyword.
            PythonModelError::PositionalOnlyParameter {
                path: "models/miswired.py".to_owned(),
                step: "only".to_owned(),
                parameter: "stg_orders".to_owned(),
                line: 29,
                column: 10,
            },
            cycle(&["ping", "pong"], 9),
            // Each step reads the next: `tri_a` reads `tri_c`.
            cycle(&["tri_a", "tri_c", "tri_b"], 17),
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
            "MG-0620", "MG-0620", "MG-0620", "MG-0621", "MG-0621", "MG-0620", "MG-0625", "MG-0622",
            "MG-0622"
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
        "`models/miswired.py` line 29 column 10: parameter `stg_orders` of step `only` is \
         positional-only; Mergus passes every input by keyword, so move it after the `/`"
    );
    assert_eq!(
        messages[8].2,
        "`models/miswired.py` line 17: steps `tri_a` -> `tri_c` -> `tri_b` -> `tri_a` form a \
         cycle"
    );
}
