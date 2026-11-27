//! MG-REQ-0412, MG-REQ-0413, MG-REQ-0415 and MG-REQ-0418 on the reads of an incremental
//! table: the start of the reads, the text of the watermark, the delete column and the
//! position that moves only on commit.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{
    Array, ArrayRef, AsArray, BooleanArray, Date32Array, Int64Array, RecordBatch, StringArray,
};
use arrow::datatypes::Int64Type;
use mergus_ports::real::RealDatabase;
use mergus_ports::{Database, DatabaseLocation, SqlValue};
use mergus_project::{ConfigFile, Source};
use mergus_sim::{DatabaseFaults, SimDatabase};
use mergus_source::{
    AttachedSource, DatabaseEngine, DeclaredTable, DeleteCheck, DeletePolicy, EngineIdentity,
    IncrementalReader, Start, Temporal, declared_tables, load_options,
};
use serde_json::{Value, json};

#[cfg(test)]
fn source(table: Value, overlap: Option<&str>) -> Source {
    let mut value = json!({ "kind": "database", "engine": "duckdb", "tables": { "sales": table } });
    if let Some(overlap) = overlap {
        value["overlap"] = json!(overlap);
    }
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
fn attach(database: &SimDatabase) -> AttachedSource {
    AttachedSource {
        connection: database.connect(&DatabaseLocation::InMemory).unwrap(),
        alias: "erp".to_owned(),
        identity: EngineIdentity {
            engine: DatabaseEngine::DuckDb,
            version: "v1.4.2".to_owned(),
        },
    }
}

#[cfg(test)]
/// A simulated source whose `sales` has the columns `columns`, `(name, DuckDB type)`.
fn describe(columns: &[(&str, &str)]) -> SimDatabase {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let names: ArrayRef = Arc::new(StringArray::from_iter_values(columns.iter().map(|c| c.0)));
    let types: ArrayRef = Arc::new(StringArray::from_iter_values(columns.iter().map(|c| c.1)));
    database.record_query(
        "SELECT column_name, column_type FROM (DESCRIBE \"erp\".\"sales\")",
        Vec::new(),
        vec![RecordBatch::try_from_iter([("column_name", names), ("column_type", types)]).unwrap()],
    );
    database
}

#[cfg(test)]
fn ids(values: &[i64]) -> RecordBatch {
    let column: ArrayRef = Arc::new(Int64Array::from(values.to_vec()));
    RecordBatch::try_from_iter([("sale_id", column)]).unwrap()
}

#[cfg(test)]
fn declared(source: &Source) -> DeclaredTable {
    declared_tables(source).remove(0)
}

#[test]
fn mg_req_0412_read_starts_at_watermark_minus_overlap() {
    let cases = [
        // `TIMESTAMP`, read as UTC, with the source's overlap.
        (
            "TIMESTAMP",
            Some("2h"),
            "2026-01-02 03:04:05.5",
            "2026-01-02 01:04:05.5",
        ),
        // `TIMESTAMPTZ`, with the default `1h`.
        (
            "TIMESTAMP WITH TIME ZONE",
            None,
            "2026-01-02 00:30:00+00",
            "2026-01-01 23:30:00+00",
        ),
        // `DATE`: the overlap in whole days, `1h` rounded up to one day.
        ("DATE", None, "2026-01-02", "2026-01-01"),
        // Any other type is read from the watermark itself (MG-REQ-0412.2).
        ("BIGINT", Some("2h"), "42", "42"),
    ];
    for (column_type, overlap, watermark, since) in cases {
        let source = source(json!({ "key": "sale_id", "incremental": "mark" }), overlap);
        let table = declared(&source);
        let database = describe(&[("sale_id", "BIGINT"), ("mark", column_type)]);
        database.record_query(
            "SELECT * FROM \"erp\".\"sales\" WHERE \"mark\" >= ?",
            vec![SqlValue::Text(since.to_owned())],
            vec![ids(&[1])],
        );
        let mut attached = attach(&database);
        let start = Start {
            watermark: Some(watermark.to_owned()),
            ..Start::default()
        };
        let mut reader = IncrementalReader::open(&mut attached, &source, &table, start).unwrap();
        assert_eq!(reader.since(), Some(since), "{column_type}");
        let chunk = reader.next_chunk().unwrap().unwrap();
        assert!(chunk.last);
        assert_eq!(chunk.rows[0].num_rows(), 1);
    }
}

#[test]
fn mg_req_0412_table_overlap_wins() {
    let source = source(json!({ "key": "id", "overlap": "30m" }), Some("2h"));
    assert_eq!(
        load_options(&source, &declared(&source)).overlap,
        Duration::from_secs(1_800)
    );
    let source = source_default();
    let options = load_options(&source, &declared(&source));
    assert_eq!(options.overlap, Duration::from_secs(3_600));
    assert_eq!(options.deletes, DeletePolicy::None);
    assert_eq!(options.delete_check, DeleteCheck::Daily);
    assert_eq!(options.delete_window, None);
}

#[cfg(test)]
fn source_default() -> Source {
    source(json!({ "key": "id" }), None)
}

/// MG-REQ-0412.1 and MG-REQ-0413: the watermark is the largest non-NULL value of the rows,
/// in the text DuckDB gives for `CAST(value AS VARCHAR)`, asked of the real DuckDB through the
/// port and compared with DuckDB's own `CAST(max(...) AS VARCHAR)`.
#[test]
fn mg_req_0413_watermark_text_is_duckdb_cast() {
    let database = RealDatabase::new(std::path::Path::new(env!("CARGO_TARGET_TMPDIR")));
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    connection.execute("SET TimeZone = 'UTC'", &[]).ok();
    for statement in [
        "CREATE SCHEMA erp",
        "CREATE TABLE erp.sales (sale_id BIGINT, ts TIMESTAMP, tz TIMESTAMPTZ, d DOUBLE, \
         day DATE, amount DECIMAL(10,2), name VARCHAR, none BIGINT)",
        "INSERT INTO erp.sales VALUES \
         (1, '2026-03-04 05:06:07.25', '2026-03-04 05:06:07.000001+00', 1e20, '2026-03-04', \
          12.50, 'abc', NULL), \
         (2, '1969-12-31 23:59:59', '2026-03-04 07:06:07+02', 0.1, '1999-01-01', 3.00, 'b', NULL), \
         (3, NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
    ] {
        connection.execute(statement, &[]).unwrap();
    }
    let mut attached = AttachedSource {
        connection,
        alias: "erp".to_owned(),
        identity: EngineIdentity {
            engine: DatabaseEngine::DuckDb,
            version: "v1.4.2".to_owned(),
        },
    };
    let rows = attached
        .connection
        .query_arrow("SELECT * FROM erp.sales", &[])
        .unwrap();
    for column in ["ts", "tz", "d", "day", "amount", "name", "none"] {
        let query = ["SELECT CAST(max(", column, ") AS VARCHAR) FROM erp.sales"].concat();
        let theirs = attached.connection.query_arrow(&query, &[]).unwrap();
        let theirs = theirs[0].column(0).as_string::<i32>();
        let theirs = (!theirs.is_null(0)).then(|| theirs.value(0).to_owned());
        let source = source(json!({ "key": "sale_id", "incremental": column }), None);
        let table = declared(&source);
        let mut reader =
            IncrementalReader::open(&mut attached, &source, &table, Start::default()).unwrap();
        assert_eq!(reader.watermark(&rows).unwrap(), theirs, "{column}");
    }
}

#[test]
fn mg_req_0413_temporal_text_round_trips() {
    for (temporal, text) in [
        (Temporal::Timestamp, "2026-03-04 05:06:07.25"),
        (Temporal::TimestampTz, "2026-03-04 05:06:07.000001+00"),
        (Temporal::Date, "2026-03-04"),
    ] {
        let micros = temporal.parse(text).unwrap();
        assert_eq!(temporal.format(micros), text);
    }
    assert_eq!(
        Temporal::TimestampTz.parse("2026-03-04 07:06:07+02:00"),
        Temporal::TimestampTz.parse("2026-03-04 05:06:07+00")
    );
}

#[test]
fn mg_req_0415_window_needs_a_date_or_time_incremental() {
    let table = json!({ "key": "sale_id", "incremental": "mark", "deletes": "compare",
                        "delete_window": "7d" });
    let source = source(table, None);
    let database = describe(&[("sale_id", "BIGINT"), ("mark", "BIGINT")]);
    let mut attached = attach(&database);
    let error =
        IncrementalReader::open(&mut attached, &source, &declared(&source), Start::default())
            .unwrap_err();
    assert_eq!(error.code(), Some("MG-0413"));
    assert_eq!(
        error.to_string(),
        "table `sales` of source `erp` declares `delete_window` but its incremental column is \
         missing or not a date or time column"
    );
}

#[test]
fn mg_req_0415_delete_column_splits_rows() {
    let source = source(
        json!({ "key": "sale_id", "deletes": "column:removed_at", "delete_check": "every_run" }),
        None,
    );
    let table = declared(&source);
    let database = describe(&[("sale_id", "BIGINT"), ("removed_at", "DATE")]);
    let removed: ArrayRef = Arc::new(Date32Array::from(vec![None, Some(20_454), None]));
    let key: ArrayRef = Arc::new(Int64Array::from(vec![1, 2, 3]));
    let rows = RecordBatch::try_from_iter([("sale_id", key), ("removed_at", removed)]).unwrap();
    database.record_query("SELECT * FROM \"erp\".\"sales\"", Vec::new(), vec![rows]);
    let mut attached = attach(&database);
    let mut reader =
        IncrementalReader::open(&mut attached, &source, &table, Start::default()).unwrap();
    assert_eq!(reader.options().delete_check, DeleteCheck::EveryRun);
    // A non-boolean column: any non-NULL value is a delete (MG-REQ-0415.2).
    let chunk = reader.next_chunk().unwrap().unwrap();
    let kept = chunk.rows[0]
        .column(0)
        .as_primitive::<Int64Type>()
        .values()
        .to_vec();
    let gone = chunk.deleted[0]
        .column(0)
        .as_primitive::<Int64Type>()
        .values()
        .to_vec();
    assert_eq!((kept, gone), (vec![1, 3], vec![2]));
    assert_eq!(chunk.deleted[0].num_columns(), 2);

    let flag: ArrayRef = Arc::new(BooleanArray::from(vec![Some(true), Some(false), None]));
    let key: ArrayRef = Arc::new(Int64Array::from(vec![1, 2, 3]));
    let rows = RecordBatch::try_from_iter([("sale_id", key), ("removed", flag)]).unwrap();
    let source = source_flag();
    let table = declared(&source);
    let database = describe(&[("sale_id", "BIGINT"), ("removed", "BOOLEAN")]);
    database.record_query("SELECT * FROM \"erp\".\"sales\"", Vec::new(), vec![rows]);
    let mut attached = attach(&database);
    let mut reader =
        IncrementalReader::open(&mut attached, &source, &table, Start::default()).unwrap();
    let chunk = reader.next_chunk().unwrap().unwrap();
    assert_eq!(
        (chunk.rows[0].num_rows(), chunk.deleted[0].num_rows()),
        (2, 1)
    );
}

#[cfg(test)]
fn source_flag() -> Source {
    source(
        json!({ "key": "sale_id", "deletes": "column:removed" }),
        None,
    )
}

#[test]
fn mg_req_0418_position_moves_only_on_commit() {
    let source = source(json!({ "key": "sale_id" }), None);
    let table = declared(&source);
    let database = describe(&[("sale_id", "BIGINT")]);
    let first = "SELECT * FROM \"erp\".\"sales\" ORDER BY \"sale_id\" LIMIT ?";
    let after =
        "SELECT * FROM \"erp\".\"sales\" WHERE \"sale_id\" > ? ORDER BY \"sale_id\" LIMIT ?";
    database.record_query(first, vec![SqlValue::Integer(2)], vec![ids(&[1, 2])]);
    database.record_query(
        after,
        vec![SqlValue::Integer(2), SqlValue::Integer(2)],
        vec![ids(&[3])],
    );
    database.record_query(
        after,
        vec![SqlValue::Integer(7), SqlValue::Integer(2)],
        vec![ids(&[8])],
    );
    let mut attached = attach(&database);
    let start = Start {
        chunk_rows: NonZeroU32::new(2),
        ..Start::default()
    };
    let mut reader = IncrementalReader::open(&mut attached, &source, &table, start).unwrap();
    let read = |chunk: mergus_source::Chunk| {
        chunk.rows[0]
            .column(0)
            .as_primitive::<Int64Type>()
            .values()
            .to_vec()
    };
    // Not committed: the same chunk is read again.
    assert_eq!(read(reader.next_chunk().unwrap().unwrap()), [1, 2]);
    assert_eq!(read(reader.next_chunk().unwrap().unwrap()), [1, 2]);
    reader.committed();
    let last = reader.next_chunk().unwrap().unwrap();
    assert!(last.last);
    assert_eq!(read(last), [3]);
    reader.committed();
    assert!(reader.next_chunk().unwrap().is_none());
}

#[test]
fn mg_req_0418_resumed_read_starts_after_key() {
    let source = source(json!({ "key": "sale_id" }), None);
    let table = declared(&source);
    let database = describe(&[("sale_id", "BIGINT")]);
    let after =
        "SELECT * FROM \"erp\".\"sales\" WHERE \"sale_id\" > ? ORDER BY \"sale_id\" LIMIT ?";
    let bound = vec![SqlValue::Integer(7), SqlValue::Integer(2)];
    database.record_query(after, bound, vec![ids(&[8])]);
    // A resumed read starts after the key it is given (MG-REQ-0418.2).
    let mut attached = attach(&database);
    let start = Start {
        chunk_rows: NonZeroU32::new(2),
        resume_after: Some(ids(&[5, 7])),
        ..Start::default()
    };
    let mut reader = IncrementalReader::open(&mut attached, &source, &table, start).unwrap();
    let chunk = reader.next_chunk().unwrap().unwrap();
    assert_eq!(
        chunk.rows[0].column(0).as_primitive::<Int64Type>().values(),
        &[8]
    );
}
