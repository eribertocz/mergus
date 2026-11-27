//! Tests of the raw tables (MG-REQ-0511, MG-REQ-0512, MG-REQ-0518), written through a storage
//! adapter in memory.

mod memory;
mod registry;
mod retention;

use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, AsArray, BooleanArray, Date32Array, Decimal128Array, Int64Array, RecordBatch,
    StringArray, TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Field, Int64Type, Schema, SchemaRef, TimeUnit};
use mergus_adapter::{VersionRef, WriteMode};
use mergus_ports::Timestamp;
use mergus_runtime::{RawError, RawLoad, write_raw};
use mergus_source::DeclaredTable;

use memory::Memory;

/// 2026-01-01T00:00:00Z, in microseconds since the epoch.
const FIRST_AT: i64 = 1_767_225_600_000_000;
/// One hour later.
const SECOND_AT: i64 = FIRST_AT + 3_600_000_000;

#[cfg(test)]
fn utc() -> DataType {
    DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into()))
}

#[cfg(test)]
/// The schema of the source table `sales`: a key, a text, a decimal, a timestamp and a date.
fn schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("sale_id", DataType::Int64, false),
        Field::new("customer", DataType::Utf8, true),
        Field::new("amount", DataType::Decimal128(18, 2), true),
        Field::new(
            "sold_at",
            DataType::Timestamp(TimeUnit::Microsecond, None),
            true,
        ),
        Field::new("day", DataType::Date32, true),
    ]))
}

#[cfg(test)]
/// Rows of `sales`, one per `(sale_id, amount in cents)`.
fn sales(rows: &[(i64, i128)]) -> RecordBatch {
    let ids: Vec<i64> = rows.iter().map(|(id, _)| *id).collect();
    let customers: Vec<String> = rows
        .iter()
        .map(|(id, _)| format!("customer {id}"))
        .collect();
    let amounts: Vec<i128> = rows.iter().map(|(_, cents)| *cents).collect();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(ids.clone())),
        Arc::new(StringArray::from(customers)),
        Arc::new(
            Decimal128Array::from(amounts)
                .with_precision_and_scale(18, 2)
                .unwrap(),
        ),
        Arc::new(TimestampMicrosecondArray::from(
            ids.iter().map(|id| FIRST_AT + id).collect::<Vec<_>>(),
        )),
        Arc::new(Date32Array::from(vec![20_454; rows.len()])),
    ];
    RecordBatch::try_new(schema(), columns).unwrap()
}

#[cfg(test)]
/// The keys `sale_id` of rows detected deleted.
fn keys(ids: &[i64]) -> RecordBatch {
    let column: ArrayRef = Arc::new(Int64Array::from(ids.to_vec()));
    RecordBatch::try_from_iter([("sale_id", column)]).unwrap()
}

#[cfg(test)]
/// A load of `sales` of source `erp`, with `key` as its key.
fn load(key: &[&str], id: &str, at: i64, rows: RecordBatch, deleted: Vec<RecordBatch>) -> RawLoad {
    RawLoad {
        source: "erp".to_owned(),
        table: DeclaredTable {
            name: "sales".to_owned(),
            key: key.iter().map(|column| (*column).to_owned()).collect(),
            incremental: None,
        },
        load_id: id.to_owned(),
        extracted_at: Timestamp::from_unix_nanos(i128::from(at) * 1_000),
        schema: schema(),
        rows: vec![rows],
        deleted,
    }
}

#[cfg(test)]
fn ints(batch: &RecordBatch, column: &str) -> Vec<i64> {
    let column = batch.column_by_name(column).unwrap();
    column.as_primitive::<Int64Type>().values().to_vec()
}

#[cfg(test)]
fn texts(batch: &RecordBatch, column: &str) -> Vec<String> {
    let column = batch.column_by_name(column).unwrap().as_string::<i32>();
    column
        .iter()
        .map(|value| value.unwrap().to_owned())
        .collect()
}

#[cfg(test)]
fn flags(batch: &RecordBatch, column: &str) -> Vec<bool> {
    let column: &BooleanArray = batch.column_by_name(column).unwrap().as_boolean();
    assert_eq!(column.null_count(), 0);
    column.iter().map(Option::unwrap).collect()
}

#[cfg(test)]
fn times(batch: &RecordBatch, column: &str) -> Vec<Option<i64>> {
    let column = batch.column_by_name(column).unwrap();
    let column = column.as_primitive::<arrow::datatypes::TimestampMicrosecondType>();
    column.iter().collect()
}

#[test]
fn mg_req_0511_raw_keeps_native_types() {
    let mut adapter = Memory::default();
    let rows = sales(&[(1, 1_050), (2, 99)]);
    write_raw(
        &mut adapter,
        &load(&["sale_id"], "l1", FIRST_AT, rows.clone(), Vec::new()),
        None,
    )
    .unwrap();
    for table in ["erp__sales", "erp__sales__changes"] {
        let stored = adapter.latest(table);
        // Every source column keeps its name, its Arrow type and its values: no text blob.
        for field in schema().fields() {
            let column = stored
                .schema()
                .field_with_name(field.name())
                .unwrap()
                .clone();
            assert_eq!(
                column.data_type(),
                field.data_type(),
                "{table}.{}",
                field.name()
            );
            let at = schema().index_of(field.name()).unwrap();
            assert_eq!(
                stored.column_by_name(field.name()).unwrap().as_ref(),
                rows.column(at).as_ref()
            );
        }
        // Nothing else but the metadata columns.
        assert_eq!(stored.num_columns(), schema().fields().len() + 4, "{table}");
    }
}

#[test]
fn mg_req_0512_raw_metadata_columns() {
    let mut adapter = Memory::default();
    let first = load(
        &["sale_id"],
        "load-1",
        FIRST_AT,
        sales(&[(1, 10), (2, 20)]),
        Vec::new(),
    );
    let version = write_raw(&mut adapter, &first, None).unwrap();
    let second = load(
        &["sale_id"],
        "load-2",
        SECOND_AT,
        sales(&[(3, 30)]),
        vec![keys(&[2])],
    );
    write_raw(&mut adapter, &second, Some(&version)).unwrap();
    for table in ["erp__sales", "erp__sales__changes"] {
        assert_metadata_schema(&adapter.latest(table), table);
    }
    metadata_values(&adapter);
}

/// MG-REQ-0512.1 to MG-REQ-0512.3: the four columns of `stored`, after the source's, with
/// their types.
#[cfg(test)]
fn assert_metadata_schema(stored: &RecordBatch, table: &str) {
    let schema = stored.schema();
    let names: Vec<&str> = schema
        .fields()
        .iter()
        .map(|field| field.name().as_str())
        .collect();
    assert_eq!(
        names[5..],
        ["_load_id", "_extracted_at", "_deleted", "_deleted_at"],
        "{table}"
    );
    let types: Vec<DataType> = schema.fields()[5..]
        .iter()
        .map(|field| field.data_type().clone())
        .collect();
    assert_eq!(types, [DataType::Utf8, utc(), DataType::Boolean, utc()]);
    assert!(!schema.field_with_name("_deleted").unwrap().is_nullable());
}

/// The metadata values after a first load of sales 1 and 2, then a load of 3 that found 2
/// deleted.
#[cfg(test)]
fn metadata_values(adapter: &Memory) {
    let changes = adapter.latest("erp__sales__changes");
    assert_eq!(ints(&changes, "sale_id"), [1, 2, 3, 2]);
    assert_eq!(
        texts(&changes, "_load_id"),
        ["load-1", "load-1", "load-2", "load-2"]
    );
    assert_eq!(
        times(&changes, "_extracted_at"),
        [
            Some(FIRST_AT),
            Some(FIRST_AT),
            Some(SECOND_AT),
            Some(SECOND_AT)
        ]
    );
    assert_eq!(flags(&changes, "_deleted"), [false, false, false, true]);
    // `_deleted_at` is NULL unless `_deleted` is true.
    assert_eq!(
        times(&changes, "_deleted_at"),
        [None, None, None, Some(SECOND_AT)]
    );
    let current = adapter.latest("erp__sales");
    assert_eq!(flags(&current, "_deleted"), [false, false]);
    assert_eq!(times(&current, "_deleted_at"), [None, None]);
}

#[test]
fn mg_req_0518_current_and_changes_views() {
    let mut adapter = Memory::default();
    let first = load(
        &["sale_id"],
        "l1",
        FIRST_AT,
        sales(&[(1, 10), (2, 20), (3, 30)]),
        Vec::new(),
    );
    let version = write_raw(&mut adapter, &first, None).unwrap();
    // MG-REQ-0518.1: one commit appends to `__changes` and replaces the current table.
    assert_eq!(adapter.commits(), 1);
    assert_eq!(
        adapter.writes,
        [
            ("erp__sales__changes".to_owned(), WriteMode::Append),
            ("erp__sales".to_owned(), WriteMode::Replace)
        ]
    );

    // Sale 2 changes, 4 is new and 3 is detected deleted.
    let second = load(
        &["sale_id"],
        "l2",
        SECOND_AT,
        sales(&[(2, 25), (4, 40)]),
        vec![keys(&[3])],
    );
    let version = write_raw(&mut adapter, &second, Some(&version)).unwrap();
    assert_eq!(version, VersionRef::new("2"));
    current_and_changes(&adapter);
    repeated_key_writes_nothing(&mut adapter, &version);
}

/// The tables after sales 1, 2 and 3, then a load where 2 changes, 4 is new and 3 is deleted.
#[cfg(test)]
fn current_and_changes(adapter: &Memory) {
    let current = adapter.latest("erp__sales");
    let mut rows: Vec<(i64, String)> = ints(&current, "sale_id")
        .into_iter()
        .zip(texts(&current, "_load_id"))
        .collect();
    rows.sort();
    assert_eq!(
        rows,
        [
            (1, "l1".to_owned()),
            (2, "l2".to_owned()),
            (4, "l2".to_owned())
        ]
    );

    // Every version and the delete; MG-REQ-0518.2: the delete carries the last known values.
    let changes = adapter.latest("erp__sales__changes");
    assert_eq!(ints(&changes, "sale_id"), [1, 2, 3, 2, 4, 3]);
    assert_eq!(
        flags(&changes, "_deleted"),
        [false, false, false, false, false, true]
    );
    let deleted = changes.slice(5, 1);
    assert_eq!(texts(&deleted, "customer"), ["customer 3"]);
    let amounts = deleted.column_by_name("amount").unwrap();
    let amounts = amounts.as_primitive::<arrow::datatypes::Decimal128Type>();
    assert_eq!(amounts.value(0), 30);
    assert_eq!(texts(&deleted, "_load_id"), ["l2"]);
    assert_eq!(times(&deleted, "_extracted_at"), [Some(SECOND_AT)]);
    assert_eq!(times(&deleted, "_deleted_at"), [Some(SECOND_AT)]);
}

/// MG-REQ-0518.3: a key returned twice fails, names it, and writes nothing.
#[cfg(test)]
fn repeated_key_writes_nothing(adapter: &mut Memory, version: &VersionRef) {
    let twice = load(
        &["sale_id"],
        "l3",
        SECOND_AT,
        sales(&[(5, 1), (5, 2)]),
        Vec::new(),
    );
    let error = write_raw(adapter, &twice, Some(version)).unwrap_err();
    assert_eq!(error.code(), Some("MG-0504"));
    let RawError::RepeatedKey {
        source_name,
        table,
        key,
    } = &error
    else {
        panic!("{error}");
    };
    assert_eq!(
        (source_name.as_str(), table.as_str(), key.as_str()),
        ("erp", "sales", "sale_id = 5")
    );
    let message = error.to_string();
    assert!(
        message.contains("erp") && message.contains("sales") && message.contains("sale_id = 5"),
        "{message}"
    );
    assert_eq!(adapter.commits(), 2);
    assert_eq!(adapter.writes.len(), 4);
}

#[test]
fn mg_req_0518_without_key_current_is_the_latest_load() {
    let mut adapter = Memory::default();
    let first = load(&[], "l1", FIRST_AT, sales(&[(1, 10), (1, 10)]), Vec::new());
    let version = write_raw(&mut adapter, &first, None).unwrap();
    let second = load(&[], "l2", SECOND_AT, sales(&[(7, 70)]), Vec::new());
    write_raw(&mut adapter, &second, Some(&version)).unwrap();
    assert_eq!(ints(&adapter.latest("erp__sales"), "sale_id"), [7]);
    assert_eq!(
        ints(&adapter.latest("erp__sales__changes"), "sale_id"),
        [1, 1, 7]
    );
}
