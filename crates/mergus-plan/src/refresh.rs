//! How each model is refreshed and whether its result is stored (MG-REQ-0651 to MG-REQ-0656,
//! MG-REQ-0661), read from its effective options.

use serde_json::Value;

use crate::period::Grain;
use crate::{PlanError, PlanModel, RefreshError};

/// How a model is refreshed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refresh {
    /// Rebuilt completely when its fingerprint changes: `refresh = "full"`, the default
    /// (MG-REQ-0651).
    Full,
    /// `incremental` with `partition_by`: only the affected periods are recomputed
    /// (MG-REQ-0652), also when it has a `key` (MG-REQ-0654.1).
    Period {
        /// The partition column.
        column: String,
        /// Its grain.
        grain: Grain,
    },
    /// `incremental` with `key` and no `partition_by`: new and changed rows are merged by key
    /// (MG-REQ-0653).
    Key,
    /// `history`: every version of each key is kept (MG-REQ-0661).
    History {
        /// The business key, its columns in order.
        key: Vec<String>,
        /// What detects a change.
        by: By,
        /// How deleted keys are treated; `ignore` by default.
        deletes: Deletes,
    },
}

/// What detects a change of a `history` model: its `by`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum By {
    /// `by = "<column>"`.
    Column(String),
    /// `by = ["<column>", ...]`.
    Columns(Vec<String>),
}

/// The `deletes` of a `history` model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deletes {
    /// `ignore`, the default (MG-REQ-0661).
    Ignore,
    /// `close`.
    Close,
    /// `tombstone`.
    Tombstone,
}

/// Where the result of a model is stored (MG-REQ-0656).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// A published model: stored where it is published.
    Published,
    /// An intermediate model with `materialize = true`: stored in the private internal zone,
    /// never in gold.
    Internal,
    /// An intermediate model without `materialize = true`: computed on the fly, not stored.
    NotStored,
}

/// Returns how `model` is refreshed, from its effective options.
///
/// # Errors
///
/// Fails with [`PlanError::Refresh`] holding one [`RefreshError`] when an `incremental` model
/// has neither `partition_by` nor `key` (MG-REQ-0654) or a `history` model lacks `key` or
/// `by` (MG-REQ-0661), and with [`PlanError::BadOption`] when an option holds a value the
/// read project never gives.
pub fn refresh(model: &PlanModel) -> Result<Refresh, PlanError> {
    let refresh = text(model, "refresh")?.unwrap_or("full");
    match refresh {
        "full" => Ok(Refresh::Full),
        "incremental" => incremental(model),
        "history" => history(model),
        _ => Err(bad(model, "refresh")),
    }
}

/// Returns where the result of `model` is stored (MG-REQ-0656).
///
/// # Errors
///
/// Fails with [`PlanError::BadOption`] when `materialize` is not a boolean.
pub fn storage(model: &PlanModel) -> Result<Storage, PlanError> {
    let materialize = match model.config.get("materialize") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(bad(model, "materialize")),
    };
    Ok(match (model.published, materialize) {
        (true, _) => Storage::Published,
        (false, true) => Storage::Internal,
        (false, false) => Storage::NotStored,
    })
}

/// The refresh of an `incremental` model (MG-REQ-0652 to MG-REQ-0654).
fn incremental(model: &PlanModel) -> Result<Refresh, PlanError> {
    if let Some(partition) = text(model, "partition_by")? {
        let (column, grain) = partition
            .split_once(':')
            .and_then(|(column, grain)| Some((column, Grain::named(grain)?)))
            .ok_or_else(|| bad(model, "partition_by"))?;
        return Ok(Refresh::Period {
            column: column.to_owned(),
            grain,
        });
    }
    if names(model, "key")?.is_some() {
        return Ok(Refresh::Key);
    }
    Err(PlanError::Refresh(vec![
        RefreshError::IncrementalWithoutPeriodOrKey {
            model: model.name.clone(),
        },
    ]))
}

/// The refresh of a `history` model (MG-REQ-0661).
fn history(model: &PlanModel) -> Result<Refresh, PlanError> {
    let key = names(model, "key")?;
    let by = match model.config.get("by") {
        None | Some(Value::Null) => None,
        Some(Value::String(column)) => Some(By::Column(column.clone())),
        Some(Value::Array(_)) => names(model, "by")?.map(By::Columns),
        Some(_) => return Err(bad(model, "by")),
    };
    let deletes = match text(model, "deletes")? {
        None | Some("ignore") => Deletes::Ignore,
        Some("close") => Deletes::Close,
        Some("tombstone") => Deletes::Tombstone,
        Some(_) => return Err(bad(model, "deletes")),
    };
    match (key, by) {
        (Some(key), Some(by)) => Ok(Refresh::History { key, by, deletes }),
        (key, by) => {
            let missing = [("key", key.is_none()), ("by", by.is_none())]
                .into_iter()
                .filter_map(|(option, missing)| missing.then_some(option))
                .collect();
            Err(PlanError::Refresh(vec![RefreshError::HistoryWithout {
                model: model.name.clone(),
                missing,
            }]))
        }
    }
}

/// The text of `option`, `None` when unset.
fn text<'a>(model: &'a PlanModel, option: &str) -> Result<Option<&'a str>, PlanError> {
    match model.config.get(option) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(bad(model, option)),
    }
}

/// The column names of `option`: one name or a non-empty list of names; `None` when unset.
fn names(model: &PlanModel, option: &str) -> Result<Option<Vec<String>>, PlanError> {
    match model.config.get(option) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(name)) => Ok(Some(vec![name.clone()])),
        Some(Value::Array(items)) if !items.is_empty() => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned))
            .collect::<Option<Vec<String>>>()
            .map(Some)
            .ok_or_else(|| bad(model, option)),
        Some(_) => Err(bad(model, option)),
    }
}

/// The internal error for `option` of `model`.
fn bad(model: &PlanModel, option: &str) -> PlanError {
    PlanError::BadOption {
        model: model.name.clone(),
        option: option.to_owned(),
    }
}
