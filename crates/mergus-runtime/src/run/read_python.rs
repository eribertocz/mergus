//! The tables a `.py` model reads, as the plan names its inputs.

use std::collections::BTreeSet;

use mergus_project::{InputSource, PythonModel};

/// The inputs of `model`, sorted: each model it reads bare, each raw table or seed as
/// `raw.<table>`. Its own steps are not inputs.
pub(crate) fn python_inputs(model: &PythonModel) -> Vec<String> {
    let inputs: BTreeSet<String> = model
        .steps
        .iter()
        .flat_map(|step| &step.inputs)
        .filter_map(|input| match &input.source {
            InputSource::Step(_) => None,
            InputSource::Model(name) => Some(name.clone()),
            InputSource::Raw(table) => Some(format!("raw.{table}")),
        })
        .collect();
    inputs.into_iter().collect()
}
