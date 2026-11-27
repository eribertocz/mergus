//! History tables: what a history model applies (MG-REQ-0662 to MG-REQ-0664, MG-REQ-0667),
//! the `as_of` macro (MG-REQ-0666) and protection from rebuilds (MG-REQ-0665).

use mergus_ports::{DatabaseConnection, Timestamp};

use crate::EngineError;
use crate::segment::quote_identifier;

/// The built-in table macro `as_of(table, ts)`: the version of each key valid at `ts`, with
/// half-open validity and without tombstones (MG-REQ-0666). Every segment session executes it
/// before the project's macros.
pub const AS_OF: &str = "CREATE MACRO as_of(t, ts) AS TABLE SELECT * FROM query_table(t) \
                         WHERE \"_valid_from\" <= ts AND ts < \"_valid_to\" \
                         AND NOT \"_is_deleted\"";

/// How a history model detects a new version of a key (MG-REQ-0662).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeDetection {
    /// `by = "<column>"`: a new version when the column's value is strictly greater than the
    /// current version's, or not NULL over a current NULL. An incoming NULL never opens one.
    /// The column's value is the new version's `_valid_from` when it is not NULL
    /// (MG-REQ-0663.2), so it must be a date or a timestamp.
    Increases(String),
    /// `by = [columns]`: a new version when any listed column `IS DISTINCT FROM` the current
    /// version's.
    Differs(Vec<String>),
}

/// What happens when a key disappears from the input (MG-REQ-0664).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deletes {
    /// `deletes = "ignore"`: nothing.
    Ignore,
    /// `deletes = "close"`: the current version ends at the detection time.
    Close,
    /// `deletes = "tombstone"`: the current version ends at the detection time, and a current
    /// version with `_is_deleted = true`, the key and NULL elsewhere starts then.
    Tombstone,
}

/// The input of a history model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryInput {
    /// A raw table, read through `raw.<source>__<table>__changes` (MG-REQ-0667): its rows
    /// carry `_load_id`, `_extracted_at`, `_deleted` and `_deleted_at`.
    Changes {
        /// The id of every load of the table recorded `ok`: only their rows are read.
        ok_loads: Vec<String>,
        /// The highest `_load_id` the previous successful run applied, `None` for the first
        /// run: only rows of greater load ids are read.
        after: Option<String>,
    },
    /// Any other input: the whole input is the current state of every key.
    Other,
}

/// One run of a history model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    /// The history table, created on the first run.
    pub table: String,
    /// The table or view of the session the model reads.
    pub input: String,
    /// The key column, `key`.
    pub key: String,
    /// `by`.
    pub by: ChangeDetection,
    /// `deletes`.
    pub deletes: Deletes,
    /// Whether the input is a changes view or any other input.
    pub input_kind: HistoryInput,
    /// The run's UTC start, kept to the microsecond: the detection time (MG-REQ-0662.1).
    pub detected_at: Timestamp,
}

/// Whether the command that rebuilds a history table carries `--rebuild-history` and its
/// confirmation (MG-REQ-0665).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebuildHistory {
    /// The rebuild was not confirmed: the history table is kept.
    Refused,
    /// `--rebuild-history` with an interactive confirmation or `--yes`.
    Confirmed,
}

/// Drops the history table `table` for a full rebuild, only when `rebuild` is
/// [`RebuildHistory::Confirmed`] (MG-REQ-0665): history older than raw retention cannot be
/// recovered.
///
/// # Errors
///
/// Fails with [`EngineError::HistoryRebuild`] (`MG-0604`) without touching the table when
/// the rebuild is not confirmed, and with [`EngineError::Port`] when DuckDB fails.
pub fn rebuild_history(
    session: &mut dyn DatabaseConnection,
    table: &str,
    rebuild: RebuildHistory,
) -> Result<(), EngineError> {
    if rebuild == RebuildHistory::Refused {
        return Err(EngineError::HistoryRebuild {
            table: table.to_owned(),
        });
    }
    let quoted = quote_identifier(table);
    // sql: identifiers quoted by quote_identifier
    session.execute(&format!("DROP TABLE IF EXISTS {quoted}"), &[])?;
    Ok(())
}
