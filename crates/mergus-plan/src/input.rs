//! What the planner receives: the models of the read project and the caller's request.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use mergus_project::{Effective, Identifier, ModelOptions};
use serde_json::Value;

/// The `max_parallel` the planner uses when the caller sets none (MG-REQ-0744.1).
pub const DEFAULT_MAX_PARALLEL: NonZeroU32 = NonZeroU32::MIN.saturating_add(3);

/// One model of the read project, as the planner needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanModel {
    /// The model name.
    pub name: String,
    /// Its stable identifier (MG-REQ-0368).
    pub model_id: String,
    /// The text of its file, CRLF turned into LF (MG-REQ-0743).
    pub code: String,
    /// Its effective option values, without origins (MG-REQ-0743).
    pub config: BTreeMap<String, Value>,
    /// The tables it reads, resolved: lowercase, a model bare, a raw table as `raw.<table>`.
    /// A Python model reads none in 0.2.
    pub inputs: BTreeSet<String>,
}

impl PlanModel {
    /// Builds the model from what `mergus-project` read: its effective options, its
    /// identifier, the text of its file and its resolved references
    /// ([`mergus_project::read_sql_references`]; none for a Python model).
    #[must_use]
    pub fn from_project(
        options: &ModelOptions,
        identifier: &Identifier,
        code: &str,
        references: &[String],
    ) -> Self {
        Self {
            name: options.name.clone(),
            model_id: identifier.as_str().to_owned(),
            code: lf_text(code),
            config: effective_config(options),
            inputs: references.iter().cloned().collect(),
        }
    }
}

/// Returns `text` with every CRLF turned into LF (MG-REQ-0743).
#[must_use]
pub fn lf_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Returns the effective option values of `options`, without origins (MG-REQ-0743): a list
/// option keeps the union order of MG-REQ-0333.
#[must_use]
pub fn effective_config(options: &ModelOptions) -> BTreeMap<String, Value> {
    options
        .options
        .iter()
        .map(|(name, effective)| {
            let value = match effective {
                Effective::Single(setting) => setting.value.clone(),
                Effective::List(items) => {
                    Value::Array(items.iter().map(|item| item.value.clone()).collect())
                }
            };
            (name.clone(), value)
        })
        .collect()
}

/// What only the caller knows (MG-REQ-0744.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanRequest {
    /// The version of Mergus making the plan.
    pub mergus_version: String,
    /// The environment the plan is for (MG-REQ-0296.5).
    pub environment: String,
    /// The most steps of one parallel group.
    pub max_parallel: NonZeroU32,
    /// The names of the selected models.
    pub selected: BTreeSet<String>,
    /// The fingerprints of the available results; none from the CLI in 0.2.
    pub available: BTreeSet<String>,
    /// The version of each raw input the caller knows, by input name (`raw.<table>`).
    pub versions: BTreeMap<String, String>,
    /// The version of each engine, by name.
    pub engines: BTreeMap<String, String>,
}

impl PlanRequest {
    /// A request for `selected` with the default `max_parallel`, no available result, no
    /// input version and no engine version.
    #[must_use]
    pub fn new(mergus_version: &str, environment: &str, selected: BTreeSet<String>) -> Self {
        Self {
            mergus_version: mergus_version.to_owned(),
            environment: environment.to_owned(),
            max_parallel: DEFAULT_MAX_PARALLEL,
            selected,
            available: BTreeSet::new(),
            versions: BTreeMap::new(),
            engines: BTreeMap::new(),
        }
    }
}
