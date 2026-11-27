//! The load options of a declared table: `overlap`, `deletes`, `delete_check` and
//! `delete_window` (MG-REQ-0412, MG-REQ-0415).

use std::time::Duration;

use mergus_ports::Timestamp;
use mergus_project::Source;
use serde_json::Value;

use crate::table::DeclaredTable;

/// The overlap of a table whose source and table declare none: `1h`.
pub const DEFAULT_OVERLAP: Duration = Duration::from_secs(3_600);

/// How deletes of a table are detected (MG-REQ-0415).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeletePolicy {
    /// `none`, or no `deletes`: nothing is detected (MG-REQ-0415.1).
    None,
    /// `column:<name>`: the source's logical-delete column (MG-REQ-0415.2).
    Column(String),
    /// `compare`: the keys of the source compared with raw (MG-REQ-0415.3).
    Compare,
}

/// How often a `compare` runs: `delete_check` (MG-REQ-0415.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteCheck {
    /// At every load.
    EveryRun,
    /// When 1 hour has passed since the last compare.
    Hourly,
    /// When 24 hours have passed since the last compare, the default.
    Daily,
    /// When 7 days have passed since the last compare.
    Weekly,
}

impl DeleteCheck {
    /// Returns whether a compare is due at `now` when the last one of the table ran at
    /// `last`, `None` when it never ran (MG-REQ-0415.3).
    #[must_use]
    pub fn is_due(self, last: Option<Timestamp>, now: Timestamp) -> bool {
        let hours: i128 = match self {
            Self::EveryRun => return true,
            Self::Hourly => 1,
            Self::Daily => 24,
            Self::Weekly => 168,
        };
        last.is_none_or(|last| now.unix_nanos() - last.unix_nanos() >= hours * 3_600_000_000_000)
    }
}

/// The load options of one declared table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadOptions {
    /// How far before the watermark a read starts: the table's `overlap`, else its source's,
    /// else [`DEFAULT_OVERLAP`] (MG-REQ-0412).
    pub overlap: Duration,
    /// How deletes are detected.
    pub deletes: DeletePolicy,
    /// How often a `compare` runs.
    pub delete_check: DeleteCheck,
    /// How far back a `compare` looks, on the incremental column; `None` for the whole table.
    pub delete_window: Option<Duration>,
}

/// Returns the load options of `table` of `source`, read from a configuration that
/// `mergus-project` has validated.
#[must_use]
pub fn load_options(source: &Source, table: &DeclaredTable) -> LoadOptions {
    let options = source
        .file
        .value
        .get("tables")
        .and_then(|tables| tables.get(&table.name));
    let text = |key: &str| {
        options
            .and_then(|options| options.get(key))
            .and_then(Value::as_str)
    };
    let overlap = text("overlap")
        .or_else(|| source.file.value.get("overlap").and_then(Value::as_str))
        .and_then(duration)
        .unwrap_or(DEFAULT_OVERLAP);
    let deletes = match text("deletes") {
        Some("compare") => DeletePolicy::Compare,
        Some(policy) => policy
            .strip_prefix("column:")
            .map_or(DeletePolicy::None, |column| {
                DeletePolicy::Column(column.to_owned())
            }),
        None => DeletePolicy::None,
    };
    let delete_check = match text("delete_check") {
        Some("every_run") => DeleteCheck::EveryRun,
        Some("hourly") => DeleteCheck::Hourly,
        Some("weekly") => DeleteCheck::Weekly,
        _ => DeleteCheck::Daily,
    };
    LoadOptions {
        overlap,
        deletes,
        delete_check,
        delete_window: text("delete_window").and_then(duration),
    }
}

/// The duration `text`: a positive count followed by `m`, `h` or `d`, as the source schema
/// checks it.
fn duration(text: &str) -> Option<Duration> {
    let unit = match text.chars().last()? {
        'd' => 86_400,
        'h' => 3_600,
        'm' => 60,
        _ => return None,
    };
    let count: u64 = text.get(..text.len().saturating_sub(1))?.parse().ok()?;
    Some(Duration::from_secs(count.saturating_mul(unit)))
}
