//! MG-REQ-0619: a step is a plain function. A Python test imports it without Mergus, calls it
//! with a LazyFrame built by hand, and gets what Mergus gets on the same data.

use mergus_engine_polars::{Frame, execute_step};
use mergus_ports::real::RealFileSystem;

use super::{batch, column, floats, python, read_ipc, run_script, scratch, step, write};

const MODEL: &str = r#"import polars as pl

def in_usd(joined):
    return joined.with_columns((pl.col("amount") * pl.col("rate")).alias("amount_usd"))
"#;

/// A plain Python test of the step, in the style of pytest, run without Mergus.
const TEST: &str = r#"import site
site.main()
import importlib.util
import sys

import polars as pl


def test_in_usd():
    spec = importlib.util.spec_from_file_location("model", sys.argv[1] if len(sys.argv) > 1 else "../models/model.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    assert "mergus" not in sys.modules
    assert sorted(name for name in vars(module) if not name.startswith("__")) == ["in_usd", "pl"]
    joined = pl.LazyFrame({"amount": [10.0, 2.0, 7.0], "rate": [2.0, 0.5, 1.0]})
    result = module.in_usd(joined)
    assert isinstance(result, pl.LazyFrame)
    result.collect().write_ipc("by_hand.arrow", compression="uncompressed")


test_in_usd()
"#;

#[test]
fn mg_req_0619_step_callable_in_pytest() {
    let root = scratch("plain");
    write(&root.join("models").join("model.py"), MODEL);
    let work = root.join("work");
    let Some(python) = python(&root, &work) else {
        return;
    };
    let tests = root.join("tests");
    write(&tests.join("test_model.py"), TEST);
    run_script(&python, TEST, &tests);
    let by_hand = read_ipc(&tests.join("by_hand.arrow"));

    let joined = floats(&["amount", "rate"]);
    let inputs = [Frame {
        name: "joined".to_owned(),
        schema: joined.clone(),
        batches: vec![batch(&joined, &[&[10.0, 2.0, 7.0], &[2.0, 0.5, 1.0]])],
    }];
    let output = execute_step(
        &python,
        &RealFileSystem,
        &step(&root, "sales", "in_usd"),
        &inputs,
        &work,
    )
    .unwrap();
    assert_eq!(*output.schema, *by_hand[0].schema());
    for name in ["amount", "rate", "amount_usd"] {
        assert_eq!(
            column(&output.batches, name),
            column(&by_hand, name),
            "{name}"
        );
    }
    assert_eq!(column(&output.batches, "amount_usd"), [20.0, 1.0, 7.0]);
}
