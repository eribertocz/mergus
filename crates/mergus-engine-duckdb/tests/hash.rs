//! MG-REQ-0638: the hash of a table does not depend on the order of its rows.

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, Decimal128Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use mergus_engine_duckdb::table_hash;

#[cfg(test)]
fn rows(ids: &[i64], names: &[&str]) -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, true),
        Field::new("name", DataType::Utf8, true),
    ]));
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(ids.to_vec())),
        Arc::new(StringArray::from(names.to_vec())),
    ];
    RecordBatch::try_new(schema, columns).unwrap()
}

#[test]
fn mg_req_0638_hashes_independent_of_row_order() {
    let ordered = [rows(&[1, 2, 3], &["a", "b", "c"])];
    let shuffled = [rows(&[3, 1], &["c", "a"]), rows(&[2], &["b"])];
    let hash = table_hash(&ordered).unwrap();
    assert_eq!(hash, table_hash(&shuffled).unwrap());
    assert_eq!(hash.len(), 64);
    let changed = [rows(&[1, 2, 3], &["a", "b", "d"])];
    assert_ne!(hash, table_hash(&changed).unwrap());
}

#[test]
fn mg_req_0638_hash_tells_types_apart() {
    let column = |array: ArrayRef| {
        let field = Field::new("v", array.data_type().clone(), true);
        RecordBatch::try_new(Arc::new(Schema::new(vec![field])), vec![array]).unwrap()
    };
    let decimal = Decimal128Array::from(vec![100])
        .with_precision_and_scale(10, 2)
        .unwrap();
    let decimal = table_hash(&[column(Arc::new(decimal))]).unwrap();
    let text = table_hash(&[column(Arc::new(StringArray::from(vec!["1.00"])))]).unwrap();
    assert_ne!(decimal, text);
}
