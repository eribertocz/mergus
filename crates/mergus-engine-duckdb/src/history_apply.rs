//! Applying one run of a history model to its history table: new versions (MG-REQ-0662),
//! validity (MG-REQ-0663), deletes (MG-REQ-0664) and the changes view (MG-REQ-0667).
//!
//! The rows a run reads are staged in `mergus_history_rows`, each with its round: its rank
//! among the rows of its key, in `_extracted_at` then `_load_id` order. Each round applies at
//! most one row per key, so a key's rows go through change detection one after another.

use arrow::array::{Array, AsArray};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Int64Type};
use arrow::record_batch::RecordBatch;
use mergus_ports::{DatabaseConnection, SqlValue};

use crate::EngineError;
use crate::history::{ChangeDetection, Deletes, History, HistoryInput};
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
/// Fails with [`EngineError::HistoryRepeatedKey`] (`MG-0605`) when an input that is not a
/// changes view has a key twice, and with [`EngineError::Port`] when DuckDB fails, for
/// example when a column is missing or `by` is not a date or a timestamp. Nothing is
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
        Err(error) => {
            session.execute("ROLLBACK", &[])?;
            Err(error)
        }
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
    create_table(session, &names)?;
    stage(session, history, &names)?;
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
    // sql: identifiers quoted by quote_identifier
    let create = format!(
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
            // sql: identifiers quoted by quote_identifier
            format!(
                "CREATE TEMP TABLE mergus_history_rows AS SELECT {columns}, \
                 COALESCE({by}CAST(\"_extracted_at\" AS TIMESTAMPTZ)) AS _mergus_from, \
                 COALESCE(\"_deleted\", false) AS _mergus_deleted, \
                 CAST(\"_deleted_at\" AS TIMESTAMPTZ) AS _mergus_at, \
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
            // sql: identifiers quoted by quote_identifier
            format!(
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
    // sql: identifiers quoted by quote_identifier
    let sql = format!(
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
    // sql: identifiers quoted by quote_identifier
    let open = format!(
        "CREATE OR REPLACE TEMP TABLE mergus_history_open AS SELECT incoming.* \
         FROM mergus_history_step AS incoming LEFT JOIN {table} AS current \
         ON incoming.{key} = current.{key} AND current.\"_is_current\" \
         AND NOT current.\"_is_deleted\" \
         WHERE NOT incoming._mergus_deleted AND (current.{key} IS NULL OR {changed})"
    );
    session.execute(&open, &[])?;
    // sql: identifiers quoted by quote_identifier
    let close = format!(
        "UPDATE {table} SET \"_valid_to\" = opened._mergus_from, \"_is_current\" = false \
         FROM mergus_history_open AS opened \
         WHERE {table}.{key} = opened.{key} AND {table}.\"_is_current\""
    );
    session.execute(&close, &[])?;
    let columns = names.columns.join(", ");
    // sql: identifiers quoted by quote_identifier
    let insert = format!(
        "INSERT INTO {table} BY NAME SELECT {columns}, _mergus_from AS \"_valid_from\", \
         getvariable('mergus_sentinel') AS \"_valid_to\", true AS \"_is_current\", \
         false AS \"_is_deleted\" FROM mergus_history_open"
    );
    session.execute(&insert, &[])?;
    Ok(())
}

/// The SQL condition under which the row `incoming` is a new version of `current`
/// (MG-REQ-0662.2, MG-REQ-0662.3).
fn changed(by: &ChangeDetection) -> String {
    match by {
        ChangeDetection::Increases(column) => {
            let column = quote_identifier(column);
            format!(
                "(incoming.{column} IS NOT NULL AND (current.{column} IS NULL \
                 OR incoming.{column} > current.{column}))"
            )
        }
        ChangeDetection::Differs(columns) => {
            let tests: Vec<String> = columns
                .iter()
                .map(|column| {
                    let column = quote_identifier(column);
                    format!("incoming.{column} IS DISTINCT FROM current.{column}")
                })
                .collect();
            format!("({})", tests.join(" OR "))
        }
    }
}

/// Stages in `mergus_history_gone` the keys whose row of the round is a delete and whose
/// current version is not a tombstone, with the row's `_deleted_at` (MG-REQ-0664.1).
fn deleted_rows(session: &mut dyn DatabaseConnection, names: &Names) -> Result<(), EngineError> {
    let Names { table, key, .. } = names;
    // sql: identifiers quoted by quote_identifier
    let gone = format!(
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
    // sql: identifiers quoted by quote_identifier
    let gone = format!(
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
    // sql: identifiers quoted by quote_identifier
    let close = format!(
        "UPDATE {table} SET \"_valid_to\" = gone._mergus_at, \"_is_current\" = false \
         FROM mergus_history_gone AS gone WHERE {table}.{key} = gone.{key} \
         AND {table}.\"_is_current\""
    );
    session.execute(&close, &[])?;
    if deletes == Deletes::Tombstone {
        // sql: identifiers quoted by quote_identifier
        let tombstone = format!(
            "INSERT INTO {table} BY NAME SELECT {key}, _mergus_at AS \"_valid_from\", \
             getvariable('mergus_sentinel') AS \"_valid_to\", true AS \"_is_current\", \
             true AS \"_is_deleted\" FROM mergus_history_gone"
        );
        session.execute(&tombstone, &[])?;
    }
    Ok(())
}

/// The single integer `sql` returns, `None` when it is NULL.
fn scalar_i64(session: &mut dyn DatabaseConnection, sql: &str) -> Result<Option<i64>, EngineError> {
    let batches = session.query_arrow(sql, &[])?;
    let Some(batch) = batches.iter().find(|batch| batch.num_rows() > 0) else {
        return Ok(None);
    };
    let Some(value) = batch.column(0).as_primitive_opt::<Int64Type>() else {
        return Ok(None);
    };
    Ok((!value.is_null(0)).then(|| value.value(0)))
}

/// The single text `sql` returns, `None` when it is NULL.
fn scalar_text(
    session: &mut dyn DatabaseConnection,
    sql: &str,
) -> Result<Option<String>, EngineError> {
    Ok(first_text(&session.query_arrow(sql, &[])?))
}

/// The first value of the first column of `batches`, as text; `None` when it is NULL or
/// there is no row.
fn first_text(batches: &[RecordBatch]) -> Option<String> {
    let batch = batches.iter().find(|batch| batch.num_rows() > 0)?;
    let value = cast(batch.column(0), &DataType::Utf8).ok()?;
    let value = value.as_string::<i32>();
    (!value.is_null(0)).then(|| value.value(0).to_owned())
}
