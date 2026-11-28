//! The tables a `.py` model reads, as the plan names its inputs.

use std::collections::BTreeMap;

use mergus_project::{InputSource, PythonModel};

/// The inputs of `model`: each model it reads bare, each raw table or seed as `raw.<table>`,
/// with the line of the `def` of its first step that reads it. Its own steps are not inputs.
pub(crate) fn python_inputs(model: &PythonModel) -> BTreeMap<String, usize> {
    let mut inputs = BTreeMap::new();
    for step in &model.steps {
        for input in &step.inputs {
            let name = match &input.source {
                InputSource::Step(_) => continue,
                InputSource::Model(name) => name.clone(),
                InputSource::Raw(table) => format!("raw.{table}"),
            };
            inputs.entry(name).or_insert(step.line);
        }
    }
    inputs
}
