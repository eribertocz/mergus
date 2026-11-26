//! MG-REQ-0402, MG-REQ-0403, MG-REQ-0404, MG-REQ-0411 and MG-REQ-0417: the reads of an
//! extraction, checked on the statements sent to the simulated `Database` port.

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};

use arrow::array::{ArrayRef, Int64Array, StringArray};
use arrow::record_batch::RecordBatch;
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Extension, LoadedExtension,
    PortError, SqlValue,
};
use mergus_project::{ConfigFile, Source};
use mergus_sim::{DatabaseFaults, SimDatabase};
use mergus_source::{
    AttachedSource, DatabaseEngine, DeclaredTable, EngineIdentity, Projection, RawTables,
    ReadOptions, declared_tables, extract_source, read_table,
};
use serde_json::{Value, json};

type Log = Arc<Mutex<Vec<(String, Vec<SqlValue>)>>>;

/// A connection that records every query it runs, with its parameters.
struct Recording {
    inner: Box<dyn DatabaseConnection>,
    queries: Log,
}

impl DatabaseConnection for Recording {
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError> {
        self.inner.execute(sql, params)
    }

    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError> {
        if let Ok(mut queries) = self.queries.lock() {
            queries.push((sql.to_owned(), params.to_vec()));
        }
        self.inner.query_arrow(sql, params)
    }

    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError> {
        self.inner.append_arrow(table, batch)
    }

    fn attach_ducklake(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.inner.attach_ducklake(catalog)
    }

    fn load_extension(&mut self, extension: Extension) -> Result<(), PortError> {
        self.inner.load_extension(extension)
    }

    fn loaded_extensions(&self) -> Vec<LoadedExtension> {
        self.inner.loaded_extensions()
    }
}

/// A source attached as `erp` to the simulated `database`, and the log of its queries.
fn attach(database: &SimDatabase) -> Result<(AttachedSource, Log), PortError> {
    let queries = Log::default();
    let inner = database.connect(&DatabaseLocation::InMemory)?;
    let source = AttachedSource {
        connection: Box::new(Recording {
            inner,
            queries: Arc::clone(&queries),
        }),
        alias: "erp".to_owned(),
        identity: EngineIdentity {
            engine: DatabaseEngine::DuckDb,
            version: "v1.4.2".to_owned(),
        },
    };
    Ok((source, queries))
}

fn source(tables: &Value) -> Source {
    let value = json!({ "kind": "database", "engine": "duckdb", "tables": tables });
    Source {
        name: "erp".to_owned(),
        file: ConfigFile {
            path: "sources/erp.toml".to_owned(),
            canonical: value.to_string(),
            value,
        },
    }
}

fn sim() -> SimDatabase {
    SimDatabase::new(1, DatabaseFaults::default())
}

/// One batch of the column `sale_id` holding `ids`.
fn ids(ids: &[i64]) -> Vec<RecordBatch> {
    let column: ArrayRef = Arc::new(Int64Array::from(ids.to_vec()));
    // An error leaves no batch, which every test that reads the rows would see.
    RecordBatch::try_from_iter([("sale_id", column)])
        .into_iter()
        .collect()
}

fn rows(batches: &[RecordBatch]) -> usize {
    batches.iter().map(RecordBatch::num_rows).sum()
}

fn table(name: &str, key: &[&str], incremental: Option<&str>) -> DeclaredTable {
    DeclaredTable {
        name: name.to_owned(),
        key: key.iter().map(|&column| column.to_owned()).collect(),
        incremental: incremental.map(str::to_owned),
    }
}

fn whole() -> ReadOptions {
    ReadOptions {
        projection: Projection::All,
        since: None,
        chunk_rows: None,
    }
}

#[test]
fn mg_req_0402_undeclared_tables_not_extracted() {
    let source = source(&json!({
        "sales": { "key": "sale_id" },
        "customers": { "key": ["region", "customer_id"], "incremental": "updated_at" },
    }));
    assert_eq!(
        declared_tables(&source),
        [
            table("customers", &["region", "customer_id"], Some("updated_at")),
            table("sales", &["sale_id"], None),
        ]
    );

    // The database also holds `invoices`; only the declared tables are read.
    let database = sim();
    database.record_query("SELECT * FROM \"erp\".\"customers\"", Vec::new(), ids(&[1]));
    database.record_query("SELECT * FROM \"erp\".\"sales\"", Vec::new(), ids(&[1, 2]));
    database.record_query("SELECT * FROM \"erp\".\"invoices\"", Vec::new(), ids(&[9]));
    let (mut attached, queries) = attach(&database).unwrap();
    let extracted = extract_source(&mut attached, &source, &|_| whole()).unwrap();
    let names: Vec<&str> = extracted.iter().map(|t| t.table.as_str()).collect();
    assert_eq!(names, ["customers", "sales"]);
    assert_eq!(rows(&extracted[1].batches), 2);
    let queried = queries.lock().unwrap();
    assert_eq!(queried.len(), 2);
    assert!(!queried.iter().any(|(sql, _)| sql.contains("invoices")));
}

#[test]
fn mg_req_0403_raw_table_names() {
    let expected = RawTables {
        current: "raw.erp__sales".to_owned(),
        changes: "raw.erp__sales__changes".to_owned(),
    };
    assert_eq!(table("sales", &[], None).raw("erp"), expected);

    let source = source(&json!({ "sales": {} }));
    let database = sim();
    database.record_query("SELECT * FROM \"erp\".\"sales\"", Vec::new(), ids(&[1]));
    let (mut attached, _) = attach(&database).unwrap();
    let extracted = extract_source(&mut attached, &source, &|_| whole()).unwrap();
    assert_eq!(extracted[0].raw, expected);
}

#[test]
fn mg_req_0404_projection_and_filter_pushed_down() {
    let database = sim();
    let sql = "SELECT \"amount\", \"sale_id\", \"updated_at\" FROM \"erp\".\"sales\" \
               WHERE \"updated_at\" >= ?";
    let since = SqlValue::Text("2026-11-25 23:00:00".to_owned());
    database.record_query(sql, vec![since.clone()], ids(&[7]));
    let (mut attached, queries) = attach(&database).unwrap();
    let sales = table("sales", &["sale_id"], Some("updated_at"));
    let options = ReadOptions {
        // The key and the incremental column travel even when the project does not use them.
        projection: Projection::Columns(vec!["amount".to_owned()]),
        since: Some(since.clone()),
        chunk_rows: None,
    };
    let chunks: Vec<_> = read_table(&mut attached, &sales, &options)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(*queries.lock().unwrap(), [(sql.to_owned(), vec![since])]);
}

#[test]
fn mg_req_0404_identifiers_quoted() {
    let database = sim();
    let sql = "SELECT \"a\"\"b\" FROM \"erp\".\"sales\"";
    database.record_query(sql, Vec::new(), ids(&[1]));
    let (mut attached, _) = attach(&database).unwrap();
    let options = ReadOptions {
        projection: Projection::Columns(vec!["a\"b".to_owned()]),
        ..whole()
    };
    let sales = table("sales", &[], None);
    let read: Result<Vec<_>, _> = read_table(&mut attached, &sales, &options).collect();
    assert_eq!(read.unwrap().len(), 1);
}

#[test]
fn mg_req_0411_full_load_without_incremental() {
    let database = sim();
    database.record_query(
        "SELECT * FROM \"erp\".\"sales\"",
        Vec::new(),
        ids(&[1, 2, 3]),
    );
    let (mut attached, queries) = attach(&database).unwrap();
    let sales = table("sales", &["sale_id"], None);
    // A mark from an earlier run filters nothing on a table without `incremental`.
    let options = ReadOptions {
        since: Some(SqlValue::Integer(2)),
        ..whole()
    };
    let chunks: Vec<_> = read_table(&mut attached, &sales, &options)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows(&chunks.concat()), 3);
    let queried = queries.lock().unwrap();
    assert_eq!(queried.len(), 1);
    assert!(!queried[0].0.contains("WHERE"));

    // An incremental table read for the first time, with no mark, is loaded completely too.
    let database = sim();
    database.record_query(
        "SELECT * FROM \"erp\".\"sales\"",
        Vec::new(),
        ids(&[1, 2, 3]),
    );
    let (mut attached, _) = attach(&database).unwrap();
    let sales = table("sales", &["sale_id"], Some("updated_at"));
    let read: Result<Vec<_>, _> = read_table(&mut attached, &sales, &whole()).collect();
    assert_eq!(rows(&read.unwrap().concat()), 3);
}

#[test]
fn mg_req_0417_keyset_pagination() {
    let database = sim();
    let first = "SELECT * FROM \"erp\".\"sales\" ORDER BY \"sale_id\" LIMIT ?";
    let next = "SELECT * FROM \"erp\".\"sales\" WHERE \"sale_id\" > ? ORDER BY \"sale_id\" LIMIT ?";
    let limit = SqlValue::Integer(2);
    database.record_query(first, vec![limit.clone()], ids(&[1, 2]));
    let after = |id| vec![SqlValue::Integer(id), limit.clone()];
    database.record_query(next, after(2), ids(&[3, 4]));
    database.record_query(next, after(4), ids(&[5]));
    let (mut attached, queries) = attach(&database).unwrap();
    let sales = table("sales", &["sale_id"], None);
    let options = ReadOptions {
        chunk_rows: NonZeroU32::new(2),
        ..whole()
    };
    let chunks: Vec<_> = read_table(&mut attached, &sales, &options)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        chunks.iter().map(|c| rows(c)).collect::<Vec<_>>(),
        [2, 2, 1]
    );
    let queried = queries.lock().unwrap();
    let expected = [
        (first.to_owned(), vec![limit.clone()]),
        (next.to_owned(), after(2)),
        (next.to_owned(), after(4)),
    ];
    assert_eq!(*queried, expected);
    assert!(!queried.iter().any(|(sql, _)| sql.contains("OFFSET")));
}

#[test]
fn mg_req_0417_keyset_with_composite_key_and_filter() {
    let database = sim();
    let base = "SELECT * FROM \"erp\".\"customers\" WHERE \"updated_at\" >= ?";
    let order = " ORDER BY \"region\", \"customer_id\" LIMIT ?";
    let first = [base, order].concat();
    let next = [base, " AND (\"region\", \"customer_id\") > (?, ?)", order].concat();
    let since = SqlValue::Integer(100);
    let limit = SqlValue::Integer(2);
    let page = |regions: &[&str], customers: &[i64]| {
        let region: ArrayRef = Arc::new(StringArray::from(regions.to_vec()));
        let customer: ArrayRef = Arc::new(Int64Array::from(customers.to_vec()));
        vec![RecordBatch::try_from_iter([("region", region), ("customer_id", customer)]).unwrap()]
    };
    database.record_query(
        &first,
        vec![since.clone(), limit.clone()],
        page(&["eu", "eu"], &[1, 5]),
    );
    let after = vec![
        since.clone(),
        SqlValue::Text("eu".to_owned()),
        SqlValue::Integer(5),
        limit.clone(),
    ];
    database.record_query(&next, after, page(&[], &[]));
    let (mut attached, queries) = attach(&database).unwrap();
    let customers = table("customers", &["region", "customer_id"], Some("updated_at"));
    let options = ReadOptions {
        projection: Projection::All,
        since: Some(since),
        chunk_rows: NonZeroU32::new(2),
    };
    let chunks: Vec<_> = read_table(&mut attached, &customers, &options)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(chunks.iter().map(|c| rows(c)).collect::<Vec<_>>(), [2, 0]);
    assert_eq!(queries.lock().unwrap().len(), 2);
}
