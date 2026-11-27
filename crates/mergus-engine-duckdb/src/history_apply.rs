//! Applying one run of a history model to its history table: new versions (MG-REQ-0662),
//! validity (MG-REQ-0663), deletes (MG-REQ-0664) and the changes view (MG-REQ-0667).
//!
//! The rows a run reads are staged in `mergus_history_rows`, each with its round: its rank
//! among the rows of its key, in `_extracted_at` then `_load_id` order. Each round applies at
//! most one row per key, so a key's rows go through change detection one after another.

use mergus_ports::{DatabaseConnection, SqlValue};

use crate::EngineError;
use crate::history::{ChangeDetection, Deletes, History, HistoryInput};
use crate::history_sql::{BY_TYPES, changed, first_text, scalar_i64, scalar_text, valid_from};
use crate::segment::quote_identifier;

/// The raw metadata columns of a changes view, never columns of a history table
/// (MG-REQ-0667.3).
const RAW_COLUMNS: [&str; 4] = ["_load_id", "_extracted_at", "_deleted", "_deleted_at"];

/// The sentinel `_valid_to` of a current version, `9999-12-31 00:00:00+00`, in microseconds
/// (MG-REQ-0663.1).
const SENTINEL_MICROS: i64 = 253_402_214_400_000_000;

/// The temporary tables of a run, dropped at its end.
const SCRATCH: [&str; 5] = [
    "mergus_history_ok",
    "mergus_history_rows",
    "mergus_history_step",
    "mergus_history_open",
    "mergus_history_gone",
];

/// Applies one run of `history` to its history table, creating the table on the first run,
/// in one transaction. Returns the highest `_load_id` applied so far for a changes view
/// (MG-REQ-0667.1): the new highest, or `after` when no row is new; `None` for any other
/// input.
///
/// # Errors
///
/// Fails with [`EngineError::HistoryRepeatedKey`] (`MG-0608`) when an input that is not a
/// changes view has a key twice, with [`EngineError::HistoryByType`] (`MG-0609`) when a
/// `by = "<column>"` column is not a `DATE`, `TIMESTAMP` or `TIMESTAMPTZ`, with
/// [`EngineError::HistoryNullKey`] (`MG-0610`) when a row it reads has a NULL key, with
/// [`EngineError::RolledBack`] when the run failed and so did its rollback, and with
/// [`EngineError::Port`] when DuckDB fails, for example when a column is missing. Nothing is
/// changed then.
pub fn apply_history(
    session: &mut dyn DatabaseConnection,
    history: &History,
) -> Result<Option<String>, EngineError> {
    session.execute("BEGIN TRANSACTION", &[])?;
    match apply(session, history) {
        Ok(applied) => {
            session.execute("COMMIT", &[])?;
            Ok(applied)
        }
        Err(error) => match session.execute("ROLLBACK", &[]) {
            Ok(_) => Err(error),
            Err(rollback) => Err(EngineError::RolledBack {
                error: Box::new(error),
                rollback,
            }),
        },
    }
}

/// The names a run's SQL uses, quoted.
struct Names {
    table: String,
    input: String,
    key: String,
    /// The model's columns: the input's, without the raw metadata columns.
    columns: Vec<String>,
}

fn apply(
    session: &mut dyn DatabaseConnection,
    history: &History,
) -> Result<Option<String>, EngineError> {
    let names = names(session, history)?;
    let micros = history.detected_at.unix_nanos().div_euclid(1_000);
    let micros = i64::try_from(micros).unwrap_or(i64::MAX);
    let set = "SET VARIABLE mergus_detected_at = CAST(? AS TIMESTAMPTZ)";
    session.execute(set, &[SqlValue::TimestampTz(micros)])?;
    let set = "SET VARIABLE mergus_sentinel = CAST(? AS TIMESTAMPTZ)";
    session.execute(set, &[SqlValue::TimestampTz(SENTINEL_MICROS)])?;
    stage(session, history, &names)?;
    null_keys(session, history, &names)?;
    create_table(session, &names)?;
    let rounds = scalar_i64(
        session,
        "SELECT CAST(max(_mergus_round) AS BIGINT) FROM mergus_history_rows",
    )?;
    for round in 1..=rounds.unwrap_or(0) {
        let step = "CREATE OR REPLACE TEMP TABLE mergus_history_step AS \
                    SELECT * FROM mergus_history_rows WHERE _mergus_round = ?";
        session.execute(step, &[SqlValue::Integer(round)])?;
        open_versions(session, history, &names)?;
        if matches!(history.input_kind, HistoryInput::Changes { .. }) {
            deleted_rows(session, &names)?;
            remove_keys(session, history.deletes, &names)?;
        }
    }
    if history.input_kind == HistoryInput::Other && history.deletes != Deletes::Ignore {
        missing_keys(session, &names)?;
        remove_keys(session, history.deletes, &names)?;
    }
    let applied = match &history.input_kind {
        HistoryInput::Changes { after, .. } => {
            let sql = "SELECT max(_mergus_load) FROM mergus_history_rows";
            scalar_text(session, sql)?.or_else(|| after.clone())
        }
        HistoryInput::Other => None,
    };
    for table in SCRATCH {
        let table = quote_identifier(table);
        // sql: identifiers quoted by quote_identifier
        session.execute(&format!("DROP TABLE IF EXISTS {table}"), &[])?;
    }
    Ok(applied)
}

/// The quoted names of `history`, with the model's columns as DuckDB describes the input.
fn names(session: &mut dyn DatabaseConnection, history: &History) -> Result<Names, EngineError> {
    let input = quote_identifier(&history.input);
    // sql: identifiers quoted by quote_identifier
    let described = session.describe(&format!("SELECT * FROM {input}"), &[])?;
    if let ChangeDetection::Increases(by) = &history.by {
        let found = described.iter().find(|column| &column.name == by);
        if let Some(column) = found.filter(|column| !BY_TYPES.contains(&column.data_type.as_str()))
        {
            return Err(EngineError::HistoryByType {
                table: history.table.clone(),
                column: by.clone(),
                data_type: column.data_type.clone(),
            });
        }
    }
    let raw = matches!(history.input_kind, HistoryInput::Changes { .. });
    let columns = described
        .iter()
        .filter(|column| !(raw && RAW_COLUMNS.contains(&column.name.as_str())))
        .map(|column| quote_identifier(&column.name))
        .collect();
    Ok(Names {
        table: quote_identifier(&history.table),
        input,
        key: quote_identifier(&history.key),
        columns,
    })
}

/// Creates the history table, when it does not exist, with the model's columns followed by
/// the validity columns (MG-REQ-0663.1).
fn create_table(session: &mut dyn DatabaseConnection, names: &Names) -> Result<(), EngineError> {
    let Names { table, input, .. } = names;
    let columns = names.columns.join(", ");
    let create = format!(
        // sql: identifiers quoted by quote_identifier
        "CREATE TABLE IF NOT EXISTS {table} AS SELECT {columns}, \
         CAST(NULL AS TIMESTAMPTZ) AS \"_valid_from\", CAST(NULL AS TIMESTAMPTZ) AS \"_valid_to\", \
         CAST(NULL AS BOOLEAN) AS \"_is_current\", CAST(NULL AS BOOLEAN) AS \"_is_deleted\" \
         FROM {input} LIMIT 0"
    );
    session.execute(&create, &[])?;
    Ok(())
}

/// Stages the rows the run reads in `mergus_history_rows`, each with the `_valid_from` it
/// would give a new version (MG-REQ-0663.2), whether it is a delete, its detection time and
/// its round.
fn stage(
    session: &mut dyn DatabaseConnection,
    history: &History,
    names: &Names,
) -> Result<(), EngineError> {
    let Names { input, key, .. } = names;
    let columns = names.columns.join(", ");
    let by = match &history.by {
        ChangeDetection::Increases(column) => {
            format!("CAST({} AS TIMESTAMPTZ), ", quote_identifier(column))
        }
        ChangeDetection::Differs(_) => String::new(),
    };
    let create = match &history.input_kind {
        HistoryInput::Changes { ok_loads, after } => {
            ok(session, ok_loads)?;
            let set = "SET VARIABLE mergus_after = CAST(? AS VARCHAR)";
            let after = after.clone().map_or(SqlValue::Null, SqlValue::Text);
            session.execute(set, &[after])?;
            format!(
                // sql: identifiers quoted by quote_identifier
                "CREATE TEMP TABLE mergus_history_rows AS SELECT {columns}, \
                 COALESCE({by}CAST(\"_extracted_at\" AS TIMESTAMPTZ)) AS _mergus_from, \
                 COALESCE(\"_deleted\", false) AS _mergus_deleted, \
                 COALESCE(CAST(\"_deleted_at\" AS TIMESTAMPTZ), \
                 getvariable('mergus_detected_at')) AS _mergus_at, \
                 CAST(\"_load_id\" AS VARCHAR) AS _mergus_load, \
                 row_number() OVER (PARTITION BY {key} \
                 ORDER BY \"_extracted_at\", \"_load_id\") AS _mergus_round \
                 FROM {input} WHERE \"_load_id\" IN (SELECT load_id FROM mergus_history_ok) \
                 AND (getvariable('mergus_after') IS NULL \
                 OR \"_load_id\" > getvariable('mergus_after'))"
            )
        }
        HistoryInput::Other => {
            unique_keys(session, history, names)?;
            format!(
                // sql: identifiers quoted by quote_identifier
                "CREATE TEMP TABLE mergus_history_rows AS SELECT {columns}, \
                 COALESCE({by}getvariable('mergus_detected_at')) AS _mergus_from, \
                 false AS _mergus_deleted, getvariable('mergus_detected_at') AS _mergus_at, \
                 CAST(NULL AS VARCHAR) AS _mergus_load, CAST(1 AS BIGINT) AS _mergus_round \
                 FROM {input}"
            )
        }
    };
    session.execute(&create, &[])?;
    Ok(())
}

/// Fails when a row the run reads has a NULL key: it matches no version (MG-REQ-0663.8).
fn null_keys(
    session: &mut dyn DatabaseConnection,
    history: &History,
    names: &Names,
) -> Result<(), EngineError> {
    let key = &names.key;
    let sql = format!(
        // sql: identifiers quoted by quote_identifier
        "SELECT CAST(count(*) AS BIGINT) FROM mergus_history_rows WHERE {key} IS NULL"
    );
    if scalar_i64(session, &sql)?.unwrap_or(0) > 0 {
        return Err(EngineError::HistoryNullKey {
            table: history.table.clone(),
            key: history.key.clone(),
        });
    }
    Ok(())
}

/// Stages the ids of the `ok` loads in `mergus_history_ok`, each bound.
fn ok(session: &mut dyn DatabaseConnection, loads: &[String]) -> Result<(), EngineError> {
    session.execute("CREATE TEMP TABLE mergus_history_ok (load_id VARCHAR)", &[])?;
    for load in loads {
        let insert = "INSERT INTO mergus_history_ok VALUES (?)";
        session.execute(insert, &[SqlValue::Text(load.clone())])?;
    }
    Ok(())
}

/// Fails when an input that is not a changes view has a key twice: its rows would have no
/// order to apply them in.
fn unique_keys(
    session: &mut dyn DatabaseConnection,
    history: &History,
    names: &Names,
) -> Result<(), EngineError> {
    let Names { input, key, .. } = names;
    let sql = format!(
        // sql: identifiers quoted by quote_identifier
        "SELECT CAST({key} AS VARCHAR) FROM {input} GROUP BY {key} HAVING count(*) > 1 \
         ORDER BY {key} LIMIT 1"
    );
    let batches = session.query_arrow(&sql, &[])?;
    if batches.iter().any(|batch| batch.num_rows() > 0) {
        let value = first_text(&batches).unwrap_or_else(|| "NULL".to_owned());
        return Err(EngineError::HistoryRepeatedKey {
            table: history.table.clone(),
            key: value,
        });
    }
    Ok(())
}

/// Opens a version for each row of the round that is not a delete and that is new by
/// MG-REQ-0662: its key has no current version that is not a tombstone, or `by` detects a
/// change. The key's current version, a tombstone included, ends where the new one starts
/// (MG-REQ-0663.3).
fn open_versions(
    session: &mut dyn DatabaseConnection,
    history: &History,
    names: &Names,
) -> Result<(), EngineError> {
    let Names { table, key, .. } = names;
    let changed = changed(&history.by);
    let from = valid_from(&history.by);
    let open = format!(
        // sql: identifiers quoted by quote_identifier
        "CREATE OR REPLACE TEMP TABLE mergus_history_open AS \
         SELECT incoming.* REPLACE ({from} AS _mergus_from) \
         FROM mergus_history_step AS incoming LEFT JOIN {table} AS current \
         ON incoming.{key} = current.{key} AND current.\"_is_current\" \
         AND NOT current.\"_is_deleted\" \
         LEFT JOIN {table} AS latest ON incoming.{key} = latest.{key} AND latest.\"_is_current\" \
         WHERE NOT incoming._mergus_deleted AND (current.{key} IS NULL OR {changed})"
    );
    session.execute(&open, &[])?;
    let close = format!(
        // sql: identifiers quoted by quote_identifier
        "UPDATE {table} SET \"_valid_to\" = opened._mergus_from, \"_is_current\" = false \
         FROM mergus_history_open AS opened \
         WHERE {table}.{key} = opened.{key} AND {table}.\"_is_current\""
    );
    session.execute(&close, &[])?;
    let columns = names.columns.join(", ");
    let insert = format!(
        // sql: identifiers quoted by quote_identifier
        "INSERT INTO {table} BY NAME SELECT {columns}, _mergus_from AS \"_valid_from\", \
         getvariable('mergus_sentinel') AS \"_valid_to\", true AS \"_is_current\", \
         false AS \"_is_deleted\" FROM mergus_history_open"
    );
    session.execute(&insert, &[])?;
    Ok(())
}

/// Stages in `mergus_history_gone` the keys whose row of the round is a delete and whose
/// current version is not a tombstone, with the row's `_deleted_at` (MG-REQ-0664.1).
fn deleted_rows(session: &mut dyn DatabaseConnection, names: &Names) -> Result<(), EngineError> {
    let Names { table, key, .. } = names;
    let gone = format!(
        // sql: identifiers quoted by quote_identifier
        "CREATE OR REPLACE TEMP TABLE mergus_history_gone AS \
         SELECT incoming.{key}, incoming._mergus_at FROM mergus_history_step AS incoming \
         JOIN {table} AS current ON incoming.{key} = current.{key} \
         AND current.\"_is_current\" AND NOT current.\"_is_deleted\" \
         WHERE incoming._mergus_deleted"
    );
    session.execute(&gone, &[])?;
    Ok(())
}

/// Stages in `mergus_history_gone` the keys with a current version that is not a tombstone
/// and that are missing from the whole input, at the detection time (MG-REQ-0664.1).
fn missing_keys(session: &mut dyn DatabaseConnection, names: &Names) -> Result<(), EngineError> {
    let Names {
        table, input, key, ..
    } = names;
    let gone = format!(
        // sql: identifiers quoted by quote_identifier
        "CREATE OR REPLACE TEMP TABLE mergus_history_gone AS \
         SELECT current.{key}, getvariable('mergus_detected_at') AS _mergus_at \
         FROM {table} AS current WHERE current.\"_is_current\" AND NOT current.\"_is_deleted\" \
         AND NOT EXISTS (SELECT 1 FROM {input} AS incoming WHERE incoming.{key} = current.{key})"
    );
    session.execute(&gone, &[])?;
    Ok(())
}

/// Applies `deletes` to the keys of `mergus_history_gone` (MG-REQ-0664).
fn remove_keys(
    session: &mut dyn DatabaseConnection,
    deletes: Deletes,
    names: &Names,
) -> Result<(), EngineError> {
    if deletes == Deletes::Ignore {
        return Ok(());
    }
    let Names { table, key, .. } = names;
    let close = format!(
        // sql: identifiers quoted by quote_identifier
        "UPDATE {table} SET \"_valid_to\" = gone._mergus_at, \"_is_current\" = false \
         FROM mergus_history_gone AS gone WHERE {table}.{key} = gone.{key} \
         AND {table}.\"_is_current\""
    );
    session.execute(&close, &[])?;
    if deletes == Deletes::Tombstone {
        let tombstone = format!(
            // sql: identifiers quoted by quote_identifier
            "INSERT INTO {table} BY NAME SELECT {key}, _mergus_at AS \"_valid_from\", \
             getvariable('mergus_sentinel') AS \"_valid_to\", true AS \"_is_current\", \
             true AS \"_is_deleted\" FROM mergus_history_gone"
        );
        session.execute(&tombstone, &[])?;
    }
    Ok(())
}
