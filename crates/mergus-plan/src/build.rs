//! Building the plan (MG-REQ-0742 to MG-REQ-0745).

use std::collections::{BTreeMap, BTreeSet};

use mergus_contracts::generated::mergus_plan_v1::{
    MergusPlan, Stale, Step, StepAction, StepReason, StepRefresh,
};
use mergus_contracts::{Contract, ValidDocument, canonical_json, validate};
use serde_json::Value;

use mergus_contracts::hash::sha256_hex;

use crate::fingerprint::{FingerprintInputs, fingerprint};
use crate::graph::Graph;
use crate::incremental::{self, Fields, Refreshes};
use crate::{PlanError, PlanModel, PlanRequest, Storage, execution};

/// The prefix of a macro among the inputs of a model, `macro:<name>` (MG-REQ-0644.2): an input
/// of its fingerprint, valued by the caller's version, but not a table the step reads.
const MACRO_INPUT: &str = "macro:";

/// A plan: a `mergus_plan` document valid against its schema, identified by its hash
/// (MG-REQ-0714). It cannot be changed once made.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    document: MergusPlan,
    valid: ValidDocument,
    storage: BTreeMap<String, Storage>,
}

impl Plan {
    /// The plan as its contract type.
    #[must_use]
    pub fn document(&self) -> &MergusPlan {
        &self.document
    }

    /// The plan, validated against the `mergus_plan` schema.
    #[must_use]
    pub fn valid(&self) -> &ValidDocument {
        &self.valid
    }

    /// The identifier of the plan (MG-REQ-0743).
    #[must_use]
    pub fn plan_id(&self) -> &str {
        &self.document.plan_id
    }

    /// Where the result of the step of `model` is stored (MG-REQ-0656), or `None` when the
    /// plan has no step of that model.
    #[must_use]
    pub fn storage(&self, model: &str) -> Option<Storage> {
        self.storage.get(model).copied()
    }

    /// The models whose step produces stored output: published, or intermediate with
    /// `materialize = true` (MG-REQ-0656).
    #[must_use]
    pub fn stored(&self) -> BTreeSet<String> {
        self.storage
            .iter()
            .filter(|(_, storage)| **storage != Storage::NotStored)
            .map(|(model, _)| model.clone())
            .collect()
    }

    /// The bytes of the plan artifact: its canonical JSON (MG-REQ-0208).
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        self.valid.hash_input()
    }
}

/// Plans the selected models of `models` for `request`.
///
/// # Errors
///
/// Fails with [`PlanError::UnknownSelected`] when a selected name is not a model of `models`,
/// with [`PlanError::Cycle`] when models read one another in a cycle, with
/// [`PlanError::Refresh`] holding every refresh error of the models (MG-REQ-0654,
/// MG-REQ-0657.2, MG-REQ-0661), with [`PlanError::BadOption`],
/// [`PlanError::UnknownBackfill`] and [`PlanError::InvertedBackfill`] for a request the CLI
/// never makes, with
/// [`PlanError::ExecutionFields`] when a step's execution fields break MG-REQ-0747.4, and with
/// [`PlanError::Emit`] when the plan does not match its contract.
pub fn plan(models: &[PlanModel], request: &PlanRequest) -> Result<Plan, PlanError> {
    let graph = Graph::new(models)?;
    if let Some(unknown) = request
        .selected
        .iter()
        .find(|name| !graph.models.contains_key(name.as_str()))
    {
        return Err(PlanError::UnknownSelected(unknown.clone()));
    }
    let refreshes = incremental::resolve(&graph, request)?;
    let fingerprints = fingerprints(&graph, request);
    let steps = steps(&graph, request, &fingerprints, &refreshes);
    execution::check(&steps)?;
    let mut document = MergusPlan {
        contract: MergusPlan::NAME.to_owned(),
        contract_version: i64::from(MergusPlan::VERSION),
        plan_id: String::new(),
        mergus_version: request.mergus_version.clone(),
        environment: request.environment.clone(),
        max_parallel: i64::from(request.max_parallel.get()),
        selected: request.selected.iter().cloned().collect(),
        steps,
        stale_upstream: stale_upstream(&graph, request, &fingerprints),
        gates: None,
    };
    document.plan_id = plan_id(&validate(&document)?);
    let valid = validate(&document)?;
    let storage = document
        .steps
        .iter()
        .filter_map(|step| {
            let storage = refreshes.storage.get(step.model.as_str())?;
            Some((step.model.clone(), *storage))
        })
        .collect();
    Ok(Plan {
        document,
        valid,
        storage,
    })
}

/// The fingerprint of every model, upstream models first (MG-REQ-0711, MG-REQ-0743).
fn fingerprints<'a>(graph: &Graph<'a>, request: &PlanRequest) -> BTreeMap<&'a str, String> {
    let parameters = BTreeMap::new();
    let mut found: BTreeMap<&str, String> = BTreeMap::new();
    for name in &graph.order {
        let Some(model) = graph.models.get(name) else {
            continue;
        };
        let inputs: BTreeMap<String, Option<String>> = model
            .inputs
            .iter()
            .map(|input| {
                let version = found
                    .get(input.as_str())
                    .or_else(|| request.versions.get(input))
                    .cloned();
                (input.clone(), version)
            })
            .collect();
        let of = FingerprintInputs {
            code: &model.code,
            config: &model.config,
            inputs: &inputs,
            parameters: &parameters,
            engines: &request.engines,
            output_contract: None,
        };
        found.insert(name, fingerprint(&of));
    }
    found
}

/// One step per selected model, cut into parallel groups (MG-REQ-0744).
fn steps(
    graph: &Graph<'_>,
    request: &PlanRequest,
    fingerprints: &BTreeMap<&str, String>,
    refreshes: &Refreshes<'_>,
) -> Vec<Step> {
    let mut by_depth: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for (name, depth) in graph.depths(&request.selected) {
        by_depth.entry(depth).or_default().push(name);
    }
    let size = usize::try_from(request.max_parallel.get()).unwrap_or(usize::MAX);
    let mut steps = Vec::new();
    let mut group: i64 = 0;
    for names in by_depth.values() {
        for chunk in names.chunks(size) {
            group += 1;
            for name in chunk {
                if let Some(model) = graph.models.get(name) {
                    let print = fingerprints.get(name).cloned().unwrap_or_default();
                    let rows = request.rows.get(*name).copied();
                    let fields = incremental::fields(name, refreshes.refresh.get(name), request);
                    steps.push(step(model, print, group, &request.available, rows, fields));
                }
            }
        }
    }
    steps
}

/// The step of `model`, whose fingerprint is `print`, in the parallel group `group`, whose
/// boundaries carry about `rows` rows, with the refresh fields `fields`. A backfilled step is
/// computed whatever the changes (MG-REQ-0657.2).
fn step(
    model: &PlanModel,
    print: String,
    group: i64,
    available: &BTreeSet<String>,
    rows: Option<u64>,
    fields: Fields,
) -> Step {
    let (segments, boundaries) = execution::fields(model, rows);
    let (action, reason) = if available.contains(&print) && !fields.backfilled {
        (StepAction::Skip, StepReason::ResultAvailable)
    } else {
        (StepAction::Compute, StepReason::NoResult)
    };
    Step {
        model: model.name.clone(),
        model_id: model.model_id.clone(),
        fingerprint: print,
        inputs: model
            .inputs
            .iter()
            .filter(|input| !input.starts_with(MACRO_INPUT))
            .cloned()
            .collect(),
        action,
        reason,
        refresh: StepRefresh::Full,
        parallel_group: group,
        incremental: fields.incremental,
        segments,
        boundaries,
        periods: fields.periods,
    }
}

/// The models not selected, upstream of a selected model, that would be computed, each with
/// the selector of the first selected model downstream of it (MG-REQ-0745).
fn stale_upstream(
    graph: &Graph<'_>,
    request: &PlanRequest,
    fingerprints: &BTreeMap<&str, String>,
) -> Vec<Stale> {
    let mut downstream: BTreeMap<&str, &str> = BTreeMap::new();
    for selected in &request.selected {
        for ancestor in graph.ancestors(selected) {
            downstream.entry(ancestor).or_insert(selected.as_str());
        }
    }
    downstream
        .into_iter()
        .filter(|(model, _)| !request.selected.contains(*model))
        .filter(|(model, _)| {
            fingerprints
                .get(model)
                .is_none_or(|print| !request.available.contains(print))
        })
        .map(|(model, first)| Stale {
            model: model.to_owned(),
            include: format!("+{first}"),
        })
        .collect()
}

/// The SHA-256 of the canonical JSON of `document` without `plan_id` (MG-REQ-0743).
fn plan_id(document: &ValidDocument) -> String {
    let mut value = document.value().clone();
    if let Value::Object(members) = &mut value {
        members.remove("plan_id");
    }
    sha256_hex(canonical_json(&value).as_bytes())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::plan;
    use crate::{PlanModel, PlanRequest};

    /// The fingerprint and step inputs of `fact_margin`, which uses the macro `safe_div`
    /// valued `hash` and reads the seed `raw.seed__countries` valued `seed`.
    fn fact_margin(hash: &str, seed: &str) -> (String, Vec<String>) {
        let inputs = ["macro:safe_div", "raw.seed__countries"];
        let model = PlanModel {
            name: "fact_margin".to_owned(),
            model_id: "mdl_aaaaaaaaaaaaaaaa".to_owned(),
            code: "SELECT safe_div(1, 2) FROM raw.seed__countries\n".to_owned(),
            config: BTreeMap::new(),
            inputs: inputs.iter().map(|input| (*input).to_owned()).collect(),
            published: false,
            steps: Vec::new(),
        };
        let selected = BTreeSet::from(["fact_margin".to_owned()]);
        let mut request = PlanRequest::new("0.4.0", "dev", selected);
        request
            .versions
            .insert("macro:safe_div".to_owned(), hash.to_owned());
        request
            .versions
            .insert("raw.seed__countries".to_owned(), seed.to_owned());
        let planned = plan(&[model], &request).unwrap();
        let step = &planned.document().steps[0];
        (step.fingerprint.clone(), step.inputs.clone())
    }

    #[test]
    fn mg_req_0644_macro_and_seed_entries_enter_the_fingerprint_only() {
        let (base, inputs) = fact_margin("sha256:aa", "sha256:cc");
        assert_eq!(inputs, ["raw.seed__countries"]);
        assert_eq!(base, fact_margin("sha256:aa", "sha256:cc").0);
        assert_ne!(base, fact_margin("sha256:bb", "sha256:cc").0);
        assert_ne!(base, fact_margin("sha256:aa", "sha256:dd").0);
    }
}
