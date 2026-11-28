//! Stages 2 and 3: the read project resolved, ordered and fingerprinted into a plan by
//! `mergus-plan`, with the complete candidates as available results (MG-REQ-0707.1).

use std::collections::BTreeSet;

use mergus_plan::{Plan, PlanError, PlanModel, PlanRequest};

use super::ReadProject;

/// The models of `project` as `mergus-plan` plans them.
#[must_use]
pub fn plan_models(project: &ReadProject) -> Vec<PlanModel> {
    project
        .models
        .iter()
        .map(|model| {
            PlanModel::from_project(
                &model.options,
                &model.identifier,
                &model.code,
                &model.inputs,
            )
        })
        .collect()
}

/// Plans `project` for `request`, with `available`, the fingerprints of the complete
/// candidates, added to its available results, so that a step whose result is a candidate
/// is `skip` with reason `result_available` (MG-REQ-0707.1). Reads no data (MG-REQ-0701).
///
/// # Errors
///
/// Fails with the [`PlanError`] of `mergus-plan`.
pub fn plan_project(
    project: &ReadProject,
    request: &PlanRequest,
    available: &BTreeSet<String>,
) -> Result<Plan, PlanError> {
    let mut request = request.clone();
    request.available.extend(available.iter().cloned());
    mergus_plan::plan(&plan_models(project), &request)
}
