//! MG-REQ-0405, MG-REQ-0407 and MG-REQ-0419: sources that are not copied, breaking changes
//! of a source table, and the read times of the tables of a source, checked on the simulated
//! `Database` port.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use arrow::array::{ArrayRef, Int64Array, StringArray};
use arrow::record_batch::RecordBatch;
use mergus_ports::{
    Clock, Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Extension,
    LoadedExtension, PortError, SqlValue, Timestamp,
};
use mergus_project::{ConfigFile, Source};
use mergus_sim::{ClockFaults, DatabaseFaults, SimClock, SimDatabase};
use mergus_source::{
    AttachedSource, ColumnChange, DatabaseEngine, DeclaredTable, EngineIdentity, Projection,
    ReadOptions, SourceColumn, breaking_changes, copied_tables, declared_tables, extract_source,
    extract_source_timed, source_columns,
};
use serde_json::{Value, json};

type Log = Arc<Mutex<Vec<String>>>;

/// A connection that records every query and moves the clock one second forward per query.
struct Recording {
    inner: Box<dyn DatabaseConnection>,
    queries: Log,
    clock: Arc<SimClock>,
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
            queries.push(sql.to_owned());
        }
        self.clock.advance(Duration::from_secs(1));
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

const START: i128 = 1_795_000_000_000_000_000;

fn clock() -> Arc<SimClock> {
    Arc::new(SimClock::new(
        1,
        Timestamp::from_unix_nanos(START),
        ClockFaults::default(),
    ))
}

fn attach(
    database: &SimDatabase,
    clock: &Arc<SimClock>,
) -> Result<(AttachedSource, Log), PortError> {
    let queries = Log::default();
    let inner = database.connect(&DatabaseLocation::InMemory)?;
    let source = AttachedSource {
        connection: Box::new(Recording {
            inner,
            queries: Arc::clone(&queries),
            clock: Arc::clone(clock),
        }),
        alias: "erp".to_owned(),
        identity: EngineIdentity {
            engine: DatabaseEngine::PostgreSql,
            version: "PostgreSQL 17.2".to_owned(),
        },
    };
    Ok((source, queries))
}

fn source(mode: Option<&str>, tables: &Value) -> Source {
    let mut value = json!({ "kind": "database", "engine": "postgresql", "tables": tables });
    if let Some(mode) = mode {
        value["mode"] = json!(mode);
        value["engine"] = json!("ducklake");
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

fn sim() -> SimDatabase {
    SimDatabase::new(1, DatabaseFaults::default())
}

fn ids(ids: &[i64]) -> Vec<RecordBatch> {
    let column: ArrayRef = Arc::new(Int64Array::from(ids.to_vec()));
    RecordBatch::try_from_iter([("sale_id", column)])
        .into_iter()
        .collect()
}

fn whole(_: &DeclaredTable) -> ReadOptions {
    ReadOptions {
        projection: Projection::All,
        since: None,
        chunk_rows: None,
    }
}

fn column(name: &str, data_type: &str) -> SourceColumn {
    SourceColumn {
        name: name.to_owned(),
        data_type: data_type.to_owned(),
    }
}

/// The answer of `DESCRIBE` for `columns`.
fn described(columns: &[(&str, &str)]) -> Vec<RecordBatch> {
    let names: ArrayRef = Arc::new(StringArray::from_iter_values(
        columns.iter().map(|(name, _)| *name),
    ));
    let types: ArrayRef = Arc::new(StringArray::from_iter_values(
        columns.iter().map(|(_, data_type)| *data_type),
    ));
    RecordBatch::try_from_iter([("column_name", names), ("column_type", types)])
        .into_iter()
        .collect()
}

const DESCRIBE_SALES: &str = "SELECT column_name, column_type FROM (DESCRIBE \"erp\".\"sales\")";
const DESCRIBE_CUSTOMERS: &str =
    "SELECT column_name, column_type FROM (DESCRIBE \"erp\".\"customers\")";
const READ_SALES: &str = "SELECT * FROM \"erp\".\"sales\"";
const READ_CUSTOMERS: &str = "SELECT * FROM \"erp\".\"customers\"";

#[test]
fn mg_req_0405_connect_and_live_not_copied() {
    let tables = json!({ "sales": { "key": "sale_id" } });
    for mode in ["connect", "live"] {
        let source = source(Some(mode), &tables);
        assert_eq!(declared_tables(&source).len(), 1);
        assert!(copied_tables(&source).is_empty());

        // The database answers every read, yet nothing is read and nothing is an error.
        let database = sim();
        database.record_query(READ_SALES, Vec::new(), ids(&[1]));
        database.record_query(DESCRIBE_SALES, Vec::new(), described(&[]));
        let clock = clock();
        let (mut attached, queries) = attach(&database, &clock).unwrap();
        assert!(
            extract_source(&mut attached, &source, &whole)
                .unwrap()
                .is_empty()
        );
        let timed =
            extract_source_timed(&mut attached, &source, &whole, &|_| Vec::new(), &*clock).unwrap();
        assert!(timed.reads.is_empty());
        assert!(timed.stopped.is_empty());
        assert!(queries.lock().unwrap().is_empty());
    }
    let copied = source(Some("copy"), &tables);
    assert_eq!(copied_tables(&copied).len(), 1);
    assert_eq!(copied_tables(&source(None, &tables)).len(), 1);
}

#[test]
fn mg_req_0407_changes_since_last_load() {
    let last = [
        column("sale_id", "BIGINT"),
        column("amount", "DECIMAL(10,2)"),
        column("note", "VARCHAR"),
    ];
    // `amount` changed type (a widening too, in phase 0.3), `note` is gone, `region` is new.
    let current = [
        column("sale_id", "BIGINT"),
        column("amount", "DECIMAL(18,2)"),
        column("region", "VARCHAR"),
    ];
    assert_eq!(
        breaking_changes(&last, &current),
        [
            ColumnChange {
                column: "amount".to_owned(),
                old_type: "DECIMAL(10,2)".to_owned(),
                new_type: Some("DECIMAL(18,2)".to_owned()),
            },
            ColumnChange {
                column: "note".to_owned(),
                old_type: "VARCHAR".to_owned(),
                new_type: None,
            },
        ]
    );
    // An added column alone is not breaking (MG-REQ-0407.3), nor is a first load.
    let added = [
        last[0].clone(),
        last[1].clone(),
        column("region", "VARCHAR"),
    ];
    assert!(breaking_changes(&last[..2], &added).is_empty());
    assert!(breaking_changes(&[], &current).is_empty());

    let dropped = mergus_source::SchemaChange {
        source_name: "erp".to_owned(),
        table: "sales".to_owned(),
        changes: breaking_changes(&last, &current)[1..].to_vec(),
    };
    assert!(
        dropped.to_string().contains("dropped or renamed"),
        "{dropped}"
    );
}

/// A database where `customers` gained a column, and where in `sales` the column `amount`
/// became `VARCHAR` and the column `note` was dropped.
fn changed_sales() -> SimDatabase {
    let database = sim();
    // `region` is added to `customers`, which is not breaking (MG-REQ-0407.3).
    let customers = described(&[("customer_id", "BIGINT"), ("region", "VARCHAR")]);
    database.record_query(DESCRIBE_CUSTOMERS, Vec::new(), customers);
    database.record_query(READ_CUSTOMERS, Vec::new(), ids(&[1]));
    let sales = described(&[("sale_id", "BIGINT"), ("amount", "VARCHAR")]);
    database.record_query(DESCRIBE_SALES, Vec::new(), sales);
    database.record_query(READ_SALES, Vec::new(), ids(&[1]));
    database
}

/// The breaking changes of `sales` in [`changed_sales`]: a type change (MG-REQ-0407.1) and a
/// dropped column, with no new type (MG-REQ-0407.2).
fn sales_changes() -> [ColumnChange; 2] {
    [
        ColumnChange {
            column: "amount".to_owned(),
            old_type: "DECIMAL(10,2)".to_owned(),
            new_type: Some("VARCHAR".to_owned()),
        },
        ColumnChange {
            column: "note".to_owned(),
            old_type: "VARCHAR".to_owned(),
            new_type: None,
        },
    ]
}

#[test]
fn mg_req_0407_source_columns_from_describe() {
    let source = source(None, &json!({ "sales": {} }));
    let (mut attached, _) = attach(&changed_sales(), &clock()).unwrap();
    assert_eq!(
        source_columns(&mut attached, &declared_tables(&source)[0]).unwrap(),
        [column("sale_id", "BIGINT"), column("amount", "VARCHAR")]
    );
}

#[test]
fn mg_req_0407_breaking_source_change_stops_table() {
    let last = [
        column("sale_id", "BIGINT"),
        column("amount", "DECIMAL(10,2)"),
        column("note", "VARCHAR"),
    ];
    let source = source(None, &json!({ "customers": {}, "sales": {} }));
    let clock = clock();
    let (mut attached, queries) = attach(&changed_sales(), &clock).unwrap();
    let last_columns = |table: &DeclaredTable| match table.name.as_str() {
        "sales" => last.to_vec(),
        _ => vec![column("customer_id", "BIGINT")],
    };
    let extraction =
        extract_source_timed(&mut attached, &source, &whole, &last_columns, &*clock).unwrap();
    // `customers`, with an added column, loads; `sales` stops before any of its data is read.
    let read: Vec<&str> = extraction
        .reads
        .iter()
        .map(|read| read.extraction.table.as_str())
        .collect();
    assert_eq!(read, ["customers"]);
    let [stopped] = extraction.stopped.as_slice() else {
        panic!("one table stops: {:?}", extraction.stopped);
    };
    assert_eq!(stopped.code(), "MG-0405");
    assert_eq!(stopped.source_name, "erp");
    assert_eq!(stopped.table, "sales");
    assert_eq!(stopped.changes, sales_changes());
    let message = stopped.to_string();
    let parts = [
        "`erp`",
        "`sales`",
        "column `amount` changed type from `DECIMAL(10,2)` to `VARCHAR`",
        "column `note` (`VARCHAR`) was dropped or renamed",
    ];
    assert!(parts.iter().all(|part| message.contains(part)), "{message}");
    assert!(!queries.lock().unwrap().iter().any(|sql| sql == READ_SALES));
}

#[test]
fn mg_req_0419_tables_share_one_snapshot() {
    // No official DuckDB page documents a snapshot shared by attached PostgreSQL tables, so
    // each read records its start and end, and the source its spread (MG-REQ-0419.3).
    let source = source(None, &json!({ "customers": {}, "sales": {} }));
    let database = sim();
    for (describe, read) in [
        (DESCRIBE_CUSTOMERS, READ_CUSTOMERS),
        (DESCRIBE_SALES, READ_SALES),
    ] {
        database.record_query(describe, Vec::new(), described(&[("sale_id", "BIGINT")]));
        database.record_query(read, Vec::new(), ids(&[1]));
    }
    let clock = clock();
    let (mut attached, _) = attach(&database, &clock).unwrap();
    let extraction =
        extract_source_timed(&mut attached, &source, &whole, &|_| Vec::new(), &*clock).unwrap();
    let second = 1_000_000_000;
    // Each query moves the clock one second: describe, read, describe, read. A read starts
    // before its `DESCRIBE` (MG-REQ-0419.1).
    let times: Vec<(i128, i128)> = extraction
        .reads
        .iter()
        .map(|read| {
            (
                read.started.unix_nanos() - START,
                read.finished.unix_nanos() - START,
            )
        })
        .collect();
    assert_eq!(times, [(0, 2 * second), (2 * second, 4 * second)]);
    assert_eq!(extraction.reads[1].columns, [column("sale_id", "BIGINT")]);
    assert_eq!(extraction.spread(), Some(Duration::from_secs(4)));
    assert!(clock.now().unix_nanos() > START);

    let empty = extract_source_timed(
        &mut attached,
        &self::source(Some("live"), &json!({})),
        &whole,
        &|_| Vec::new(),
        &*clock,
    )
    .unwrap();
    assert_eq!(empty.spread(), None);
}
