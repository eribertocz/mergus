//! A source table answered by the simulated `Database` port, and the stores of one process.

use std::sync::Arc;
use std::time::Duration;

use arrow::array::TimestampMicrosecondArray;
use arrow::array::{ArrayRef, BooleanArray, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use mergus_ports::{Database, DatabaseLocation, SqlValue};
use mergus_project::{ConfigFile, Source};
use mergus_sim::{DatabaseFaults, SimDatabase};
use mergus_source::{AttachedSource, DatabaseEngine, DeclaredTable, EngineIdentity, Temporal};
use serde_json::{Value, json};

/// 2026-01-01T00:00:00Z, in microseconds since the epoch.
pub const DAY_ONE: i64 = 1_767_225_600_000_000;
/// One hour, in microseconds.
pub const HOUR: i64 = 3_600_000_000;

/// One row of the source table `erp.sales`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: i64,
    pub amount: i64,
    /// `updated_at`, in hours after [`DAY_ONE`]; `None` is NULL.
    pub hour: Option<i64>,
    /// `removed`, the logical-delete column.
    pub removed: bool,
}

#[cfg(test)]
/// A row of `sales`, not removed.
pub fn row(id: i64, amount: i64, hour: Option<i64>) -> Row {
    Row {
        id,
        amount,
        hour,
        removed: false,
    }
}

#[cfg(test)]
/// The schema of `sales`.
pub fn schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("sale_id", DataType::Int64, false),
        Field::new("amount", DataType::Int64, true),
        Field::new(
            "updated_at",
            DataType::Timestamp(TimeUnit::Microsecond, None),
            true,
        ),
        Field::new("removed", DataType::Boolean, true),
    ]))
}

#[cfg(test)]
/// `rows` as one batch of `sales`.
pub fn batch(rows: &[Row]) -> RecordBatch {
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from_iter_values(rows.iter().map(|row| row.id))),
        Arc::new(Int64Array::from_iter_values(
            rows.iter().map(|row| row.amount),
        )),
        Arc::new(TimestampMicrosecondArray::from_iter(
            rows.iter()
                .map(|row| row.hour.map(|hour| DAY_ONE + hour * HOUR)),
        )),
        Arc::new(BooleanArray::from_iter(
            rows.iter().map(|row| Some(row.removed)),
        )),
    ];
    RecordBatch::try_new(schema(), columns).unwrap()
}

#[cfg(test)]
/// The text DuckDB gives for the `TIMESTAMP` `hour` hours after [`DAY_ONE`].
pub fn at(hour: i64) -> String {
    Temporal::Timestamp.format(DAY_ONE + hour * HOUR)
}

#[cfg(test)]
/// The source `erp` declaring `sales` with `options`, keyed by `sale_id` and incremental on
/// `updated_at` unless `options` say otherwise.
pub fn source(options: Value) -> Source {
    let mut table = json!({ "key": "sale_id", "incremental": "updated_at" });
    for (name, value) in options.as_object().unwrap() {
        if value.is_null() {
            table.as_object_mut().unwrap().remove(name);
        } else {
            table[name] = value.clone();
        }
    }
    let value = json!({ "kind": "database", "engine": "duckdb", "tables": { "sales": table } });
    Source {
        name: "erp".to_owned(),
        file: ConfigFile {
            path: "sources/erp.toml".to_owned(),
            canonical: value.to_string(),
            value,
        },
    }
}

#[cfg(test)]
/// The declared table `sales` of `source`.
pub fn declared(source: &Source) -> DeclaredTable {
    mergus_source::declared_tables(source).remove(0)
}

const FROM: &str = "\"erp\".\"sales\"";

#[cfg(test)]
/// The source holding `rows`, attached as `erp` to a new simulated database that answers
/// every statement a load of `table` can send: its `DESCRIBE`, its reads from any `since`
/// a watermark of these rows gives, by chunks of `chunk` after any key, and its compares,
/// within `window` back from `now` when given. `describe` names the columns the source has.
pub fn attach(
    rows: &[Row],
    chunk: Option<u32>,
    overlap: Duration,
    window: Option<(Duration, i64)>,
    describe: &[&str],
) -> AttachedSource {
    attach_with(rows, chunk, overlap, window, describe, Vec::new())
}

/// An answer recorded after the others, so that it wins: `(sql, params, rows)`.
pub type Answer = (String, Vec<SqlValue>, Vec<RecordBatch>);

#[cfg(test)]
/// [`attach`], with `first` answered in place of what the rows give.
pub fn attach_with(
    rows: &[Row],
    chunk: Option<u32>,
    overlap: Duration,
    window: Option<(Duration, i64)>,
    describe: &[&str],
    first: Vec<Answer>,
) -> AttachedSource {
    attach_after(rows, &[], chunk, overlap, window, describe, first)
}

#[cfg(test)]
/// [`attach_with`], also answering the reads from the watermarks that the `earlier` hours,
/// those of rows that earlier loads read, give.
pub fn attach_after(
    rows: &[Row],
    earlier: &[i64],
    chunk: Option<u32>,
    overlap: Duration,
    window: Option<(Duration, i64)>,
    describe: &[&str],
    first: Vec<Answer>,
) -> AttachedSource {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    record_describe(&database, describe);
    record_texts(&database, rows);
    let mut sorted = rows.to_vec();
    sorted.sort_by_key(|row| row.id);
    let mut sinces = vec![None];
    let hours = sorted.iter().filter_map(|row| row.hour);
    for hour in hours.chain(earlier.iter().copied()) {
        let since = Temporal::Timestamp.minus(&at(hour), overlap).unwrap();
        sinces.push(Some(since));
        // A read from the watermark itself, which a test sees if the overlap is dropped.
        sinces.push(Some(at(hour)));
    }
    for since in &sinces {
        let kept: Vec<Row> = sorted
            .iter()
            .filter(|row| since.as_ref().is_none_or(|since| newer(row, since)))
            .copied()
            .collect();
        record_reads(&database, &kept, &sorted, since.as_deref(), chunk);
        record_counts(&database, &kept, &sorted, since.as_deref());
    }
    record_compare(&database, &sorted, window);
    // A statement recorded again answers with its last recording.
    for (sql, params, rows) in first {
        database.record_query(sql, params, rows);
    }
    let connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    AttachedSource {
        connection,
        alias: "erp".to_owned(),
        identity: EngineIdentity {
            engine: DatabaseEngine::DuckDb,
            version: "v1.4.2".to_owned(),
        },
    }
}

#[cfg(test)]
/// Records DuckDB's `CAST(value AS VARCHAR)` of each `updated_at` of `rows`: the text of a
/// watermark or of the oldest late row (MG-REQ-0413).
fn record_texts(database: &SimDatabase, rows: &[Row]) {
    let sql = "SELECT CAST(COALESCE((SELECT \"updated_at\" FROM \"erp\".\"sales\" WHERE false), ?) \
               AS VARCHAR)";
    for hour in rows.iter().filter_map(|row| row.hour) {
        let text: ArrayRef = Arc::new(StringArray::from(vec![at(hour)]));
        let answer = RecordBatch::try_from_iter([("text", text)]).unwrap();
        database.record_query(sql, vec![SqlValue::Text(at(hour))], vec![answer]);
    }
}

#[cfg(test)]
/// Records the `DESCRIBE` of `sales` with the columns `describe`.
fn record_describe(database: &SimDatabase, describe: &[&str]) {
    let types = [
        ("sale_id", "BIGINT"),
        ("amount", "BIGINT"),
        ("updated_at", "TIMESTAMP"),
        ("removed", "BOOLEAN"),
    ];
    let types: Vec<_> = types
        .iter()
        .filter(|(name, _)| describe.contains(name))
        .collect();
    let names: ArrayRef = Arc::new(StringArray::from_iter_values(types.iter().map(|t| t.0)));
    let kinds: ArrayRef = Arc::new(StringArray::from_iter_values(types.iter().map(|t| t.1)));
    database.record_query(
        ["SELECT column_name, column_type FROM (DESCRIBE ", FROM, ")"].concat(),
        Vec::new(),
        vec![RecordBatch::try_from_iter([("column_name", names), ("column_type", kinds)]).unwrap()],
    );
}

#[cfg(test)]
/// Whether `row` is not older than `since`.
fn newer(row: &Row, since: &str) -> bool {
    let since = Temporal::Timestamp.parse(since).unwrap();
    row.hour.is_some_and(|hour| DAY_ONE + hour * HOUR >= since)
}

#[cfg(test)]
/// Records the reads of `kept`, the rows from `since`, whole and by chunks after each key.
fn record_reads(
    database: &SimDatabase,
    kept: &[Row],
    all: &[Row],
    since: Option<&str>,
    chunk: Option<u32>,
) {
    let mut condition = Vec::new();
    let mut params = Vec::new();
    if let Some(since) = since {
        condition.push("\"updated_at\" >= ?");
        params.push(SqlValue::Text(since.to_owned()));
    }
    let whole = with_where("SELECT * FROM \"erp\".\"sales\"", &condition);
    database.record_query(whole, params.clone(), vec![batch(kept)]);
    let Some(chunk) = chunk else {
        return;
    };
    let size = usize::try_from(chunk).unwrap();
    let limit = SqlValue::Integer(i64::from(chunk));
    let first =
        with_where("SELECT * FROM \"erp\".\"sales\"", &condition) + " ORDER BY \"sale_id\" LIMIT ?";
    let first_rows: Vec<Row> = kept.iter().take(size).copied().collect();
    database.record_query(
        first,
        [params.clone(), vec![limit.clone()]].concat(),
        vec![batch(&first_rows)],
    );
    let mut after = condition.clone();
    after.push("\"sale_id\" > ?");
    for key in all {
        let sql =
            with_where("SELECT * FROM \"erp\".\"sales\"", &after) + " ORDER BY \"sale_id\" LIMIT ?";
        let rows: Vec<Row> = kept
            .iter()
            .filter(|row| row.id > key.id)
            .take(size)
            .copied()
            .collect();
        let bound = [
            params.clone(),
            vec![SqlValue::Integer(key.id), limit.clone()],
        ]
        .concat();
        database.record_query(sql, bound, vec![batch(&rows)]);
    }
}

#[cfg(test)]
/// Records the counts of `kept`, the rows from `since`, through each key of `all` and whole:
/// the rows a resumed load had read (MG-REQ-0514).
fn record_counts(database: &SimDatabase, kept: &[Row], all: &[Row], since: Option<&str>) {
    let mut conditions = Vec::new();
    let mut params = Vec::new();
    if let Some(since) = since {
        conditions.push("\"updated_at\" >= ?");
        params.push(SqlValue::Text(since.to_owned()));
    }
    let count = |rows: usize| {
        let column: ArrayRef = Arc::new(Int64Array::from(vec![i64::try_from(rows).unwrap()]));
        vec![RecordBatch::try_from_iter([("count_star()", column)]).unwrap()]
    };
    let select = "SELECT count(*) FROM \"erp\".\"sales\"";
    database.record_query(
        with_where(select, &conditions),
        params.clone(),
        count(kept.len()),
    );
    let mut through = conditions.clone();
    through.push("\"sale_id\" <= ?");
    for key in all {
        let rows = kept.iter().filter(|row| row.id <= key.id).count();
        let bound = [params.clone(), vec![SqlValue::Integer(key.id)]].concat();
        database.record_query(with_where(select, &through), bound, count(rows));
    }
}

#[cfg(test)]
/// Records the compare of the key and incremental columns, within `window` back from `now`.
fn record_compare(database: &SimDatabase, rows: &[Row], window: Option<(Duration, i64)>) {
    let keys = |rows: &[Row]| {
        let batch = batch(rows);
        vec![batch.project(&[0, 2]).unwrap()]
    };
    let sql = "SELECT \"sale_id\", \"updated_at\" FROM \"erp\".\"sales\"";
    database.record_query(sql, Vec::new(), keys(rows));
    if let Some((window, now)) = window {
        let since = Temporal::Timestamp
            .minus(&Temporal::Timestamp.format(now), window)
            .unwrap();
        let inside: Vec<Row> = rows
            .iter()
            .filter(|row| newer(row, &since))
            .copied()
            .collect();
        database.record_query(
            format!("{sql} WHERE \"updated_at\" >= ?"),
            vec![SqlValue::Text(since)],
            keys(&inside),
        );
    }
}

#[cfg(test)]
fn with_where(select: &str, conditions: &[&str]) -> String {
    if conditions.is_empty() {
        return select.to_owned();
    }
    format!("{select} WHERE {}", conditions.join(" AND "))
}
