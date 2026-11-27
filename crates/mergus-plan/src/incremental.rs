//! The refresh fields of the steps (MG-REQ-0652.3, MG-REQ-0653.1, MG-REQ-0657, MG-REQ-0747.4)
//! and where each result is stored (MG-REQ-0656).

use std::collections::BTreeMap;

use mergus_contracts::generated::mergus_plan_v1::{Incremental, IncrementalMode, Period};

use crate::graph::Graph;
use crate::period::{Range, merge};
use crate::{PlanError, PlanRequest, Refresh, RefreshError, Storage, refresh, storage};

/// The refresh and storage of every model.
pub(crate) struct Refreshes<'a> {
    /// The refresh of each model, by name.
    pub(crate) refresh: BTreeMap<&'a str, Refresh>,
    /// Where each model's result is stored, by name.
    pub(crate) storage: BTreeMap<&'a str, Storage>,
}

/// The refresh fields of one step.
pub(crate) struct Fields {
    /// `step.incremental`.
    pub(crate) incremental: Option<Incremental>,
    /// `step.periods`.
    pub(crate) periods: Option<Vec<Period>>,
    /// Whether the step is backfilled, so computed whatever the changes (MG-REQ-0657.2).
    pub(crate) backfilled: bool,
}

/// Reads the refresh and storage of every model of `graph`, and checks the backfills of
/// `request`.
///
/// # Errors
///
/// Fails with [`PlanError::Refresh`] holding every refresh error, in model name order
/// (MG-REQ-0654, MG-REQ-0657.2, MG-REQ-0661); with [`PlanError::UnknownBackfill`] when a
/// backfill names a model that is not selected; with [`PlanError::InvertedBackfill`] when one
/// ends before it starts; and with [`PlanError::BadOption`].
pub(crate) fn resolve<'a>(
    graph: &Graph<'a>,
    request: &PlanRequest,
) -> Result<Refreshes<'a>, PlanError> {
    if let Some(unknown) = request
        .backfill
        .keys()
        .find(|name| !request.selected.contains(*name))
    {
        return Err(PlanError::UnknownBackfill(unknown.clone()));
    }
    if let Some((name, _)) = request
        .backfill
        .iter()
        .find(|(_, backfill)| backfill.to < backfill.from)
    {
        return Err(PlanError::InvertedBackfill(name.clone()));
    }
    let mut found = Refreshes {
        refresh: BTreeMap::new(),
        storage: BTreeMap::new(),
    };
    let mut errors = Vec::new();
    for (name, model) in &graph.models {
        found.storage.insert(name, storage(model)?);
        match refresh(model) {
            Ok(refresh) => {
                let by_period = matches!(refresh, Refresh::Period { .. });
                if request.backfill.contains_key(*name) && !by_period {
                    errors.push(RefreshError::BackfillWithoutPeriods {
                        model: model.name.clone(),
                    });
                }
                found.refresh.insert(name, refresh);
            }
            Err(PlanError::Refresh(more)) => errors.extend(more),
            Err(other) => return Err(other),
        }
    }
    if errors.is_empty() {
        Ok(found)
    } else {
        Err(PlanError::Refresh(errors))
    }
}

/// The refresh fields of the step of `name`, refreshed as `refresh`, for `request`.
pub(crate) fn fields(name: &str, refresh: Option<&Refresh>, request: &PlanRequest) -> Fields {
    match refresh {
        Some(Refresh::Period { grain, .. }) => {
            let mut ranges: Vec<Range> = request.affected.get(name).cloned().unwrap_or_default();
            let backfill = request.backfill.get(name);
            if let Some(backfill) = backfill
                && let Some(range) = Range::new(
                    grain.period(backfill.from).start(),
                    grain.period(backfill.to).end(),
                )
            {
                ranges.push(range);
            }
            Fields {
                incremental: Some(Incremental {
                    mode: IncrementalMode::Period,
                }),
                periods: Some(merge(ranges).into_iter().map(period).collect()),
                backfilled: backfill.is_some(),
            }
        }
        Some(Refresh::Key) => Fields {
            incremental: Some(Incremental {
                mode: IncrementalMode::Key,
            }),
            periods: None,
            backfilled: false,
        },
        Some(Refresh::Full | Refresh::History { .. }) | None => Fields {
            incremental: None,
            periods: None,
            backfilled: false,
        },
    }
}

/// `range` as a contract period.
fn period(range: Range) -> Period {
    Period {
        start: range.start().to_string(),
        end: range.end().to_string(),
    }
}
