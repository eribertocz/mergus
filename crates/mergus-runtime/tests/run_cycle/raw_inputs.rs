//! A `.sql` model reads the raw tables of a source and of a seed, attached encrypted and
//! read-only at the one snapshot of raw the run took after its extractions (MG-REQ-0704,
//! MG-REQ-0515). It runs on the real DuckDB, and returns at once when an extension is not
//! cached and cannot be downloaded, except in CI (`CI` set), where that fails the test.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{ArrayRef, AsArray, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Int64Type, Schema, SchemaRef};
use arrow::ipc::reader::FileReader;
use mergus_adapter::{AdapterError, StorageAdapter, WriteMode};
use mergus_adapter_ducklake::{DuckLakeAdapter, DuckLakeError};
use mergus_ports::real::{RealClock, RealDatabase, RealEnvironment, RealFileSystem, RealRandom};
use mergus_ports::{Environment, FileSystem, PortError, Random, Timestamp};
use mergus_runtime::{
    CANDIDATES, CancelToken, Catalog, Clients, Executed, ModelWork, Profile, RESULT, RawError,
    RawLoad, Run, RunError, SqlWork, StepFailure, Work, Zone, ZoneStorage, open_raw, raw_catalog,
    raw_snapshot, write_raw,
};
use mergus_sim::SimFileSystem;
use mergus_source::DeclaredTable;

use crate::fixture::{at, cleared, fingerprint, plan, project, read, root};

/// The model: each order with the name of its country.
const NAMED: &str = "SELECT o.id, c.name FROM raw.app__orders AS o \
                     JOIN raw.seed__countries AS c ON o.country = c.code";

/// A macro that tries to write to raw.
const INTRUDER: &str = "CREATE TABLE raw.intruder AS SELECT 1 AS id";

#[test]
fn mg_req_0704_sql_segment_reads_raw_read_only() {
    let in_ci = RealEnvironment.var("CI").is_some();
    let Some(cache_dir) = RealEnvironment.cache_dir() else {
        assert!(!in_ci, "no user cache directory in CI");
        return;
    };
    let database = RealDatabase::new(&cache_dir);
    let storage = raw_storage(&scratch());
    match open_raw(&database, &storage, root(), None, clock()) {
        Ok(adapter) => drop(adapter),
        Err(error) if extension_unavailable(&error) => {
            assert!(!in_ci, "an extension is unavailable in CI: {error}");
            return;
        }
        Err(error) => panic!("{error}"),
    }
    let files = project(&[
        (
            "project.toml",
            "name = \"sales\"\n\n[seeds.countries]\n\
             columns = { code = \"VARCHAR\", name = \"VARCHAR\" }\n",
        ),
        ("seeds/countries.csv", "code,name\nPE,Peru\nCL,Chile\n"),
        (
            "sources/app.toml",
            "kind = \"database\"\nengine = \"duckdb\"\nlocation = \"data/app.duckdb\"\n\
             [tables.orders]\nkey = \"id\"\n",
        ),
        ("models/named_orders.sql", NAMED),
    ]);
    run_reads_raw(&files, &database, &storage);
    segment_cannot_write_raw(&files, &database, &storage);
}

/// The run extracts the orders and the seed into raw, takes its snapshot, then commits one
/// more order: the model reads raw at the snapshot, so its candidate holds the two orders
/// of the extraction only.
#[cfg(test)]
fn run_reads_raw(files: &SimFileSystem, database: &RealDatabase, storage: &ZoneStorage) {
    let project = read(files);
    let run = Run::begin(files, root(), "run1", CancelToken::new()).unwrap();
    let planned = plan(&project, run.available(), 1);
    let adopted = run.adopt(&planned).unwrap();
    let taken = std::cell::Cell::new(None);
    let mut work = named_work(files, database, storage, Vec::new());
    work.extractions.push(Box::new(|_cancel: &CancelToken| {
        let mut raw = open_raw(database, storage, root(), None, clock()).unwrap();
        write_raw(&mut raw, &orders(&[(1, "PE"), (2, "CL")]), None).unwrap();
        write_seed(&mut raw);
        Ok(())
    }));
    work.raw_snapshot = Some(Box::new(|| {
        let mut raw = open_raw(database, storage, root(), None, clock()).unwrap();
        let snapshot = raw_snapshot(&raw).map_err(failure)?;
        taken.set(snapshot);
        // A commit after the snapshot, which the segment must not see.
        let previous = raw.latest().unwrap();
        write_raw(&mut raw, &orders(&[(3, "PE")]), previous.as_ref()).unwrap();
        Ok(snapshot)
    }));
    let executed = run.execute(&adopted, &cleared(), &mut work).unwrap();
    drop(work);
    let report = run.finish(Ok(executed.clone()));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let Executed::Finished { rows } = executed else {
        panic!("not finished");
    };
    assert_eq!(rows.get("named_orders"), Some(&2));
    let result = at(CANDIDATES)
        .join(fingerprint(&planned, "named_orders"))
        .join(RESULT);
    let mut named = named_rows(files, &result);
    named.sort();
    assert_eq!(
        named,
        [(1, "Peru".to_owned()), (2, "Chile".to_owned())],
        "the segment reads raw at the run's snapshot"
    );
    assert!(
        taken.get().is_some(),
        "raw has a snapshot after the extractions"
    );
}

/// A segment whose macro writes to raw fails, even with raw attached at its latest snapshot,
/// and raw keeps its snapshots and tables.
#[cfg(test)]
fn segment_cannot_write_raw(files: &SimFileSystem, database: &RealDatabase, storage: &ZoneStorage) {
    let project = read(files);
    let run = Run::begin(files, root(), "run2", CancelToken::new()).unwrap();
    // Nothing is available, so that the model runs again.
    let planned = plan(&project, &std::collections::BTreeSet::new(), 1);
    let adopted = run.adopt(&planned).unwrap();
    let before = latest(database, storage);
    let mut work = named_work(files, database, storage, vec![INTRUDER.to_owned()]);
    work.raw_snapshot = Some(Box::new(|| Ok(None)));
    let outcome = run.execute(&adopted, &cleared(), &mut work);
    drop(work);
    let error = outcome.clone().err().unwrap();
    assert!(matches!(error, RunError::Step(_)), "{error}");
    let _ = run.finish(outcome);
    assert_eq!(latest(database, storage), before, "raw was written");
    let raw = open_raw(database, storage, root(), None, clock()).unwrap();
    let version = raw.latest().unwrap().unwrap();
    assert!(raw.read_at("intruder", &version).is_err());
}

/// The work of the run: the model `named_orders`, which reads raw directly, with `macros`.
#[cfg(test)]
fn named_work<'a>(
    files: &'a SimFileSystem,
    database: &'a RealDatabase,
    storage: &ZoneStorage,
    macros: Vec<String>,
) -> Work<'a> {
    let output: SchemaRef = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, true),
        Field::new("name", DataType::Utf8, true),
    ]));
    let named = SqlWork {
        database,
        files,
        sql: NAMED.to_owned(),
        params: Vec::new(),
        macros,
        inputs: Vec::new(),
        catalogs: vec![raw_catalog(storage, root(), None).unwrap()],
        output,
    };
    let mut models: std::collections::BTreeMap<String, Box<dyn ModelWork + 'a>> =
        std::collections::BTreeMap::new();
    models.insert("named_orders".to_owned(), Box::new(named));
    Work {
        extractions: Vec::new(),
        raw_snapshot: None,
        models,
        bytes_scanned: 0,
    }
}

/// The rows `(id, name)` of the Arrow IPC file at `path`.
#[cfg(test)]
fn named_rows(files: &SimFileSystem, path: &Path) -> Vec<(i64, String)> {
    let reader = FileReader::try_new(files.open(path).unwrap(), None).unwrap();
    let mut rows = Vec::new();
    for batch in reader {
        let batch = batch.unwrap();
        let ids = batch.column(0).as_primitive::<Int64Type>();
        let names = batch.column(1).as_string::<i32>();
        for row in 0..batch.num_rows() {
            rows.push((ids.value(row), names.value(row).to_owned()));
        }
    }
    rows
}

/// The latest snapshot of raw.
#[cfg(test)]
fn latest(database: &RealDatabase, storage: &ZoneStorage) -> Option<i64> {
    let raw = open_raw(database, storage, root(), None, clock()).unwrap();
    raw_snapshot(&raw).unwrap()
}

/// The load of `app.orders` holding `rows`, each `(id, country)`.
#[cfg(test)]
fn orders(rows: &[(i64, &str)]) -> RawLoad {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("country", DataType::Utf8, false),
    ]));
    let ids: ArrayRef = Arc::new(Int64Array::from_iter_values(rows.iter().map(|row| row.0)));
    let countries: ArrayRef = Arc::new(StringArray::from_iter_values(rows.iter().map(|row| row.1)));
    RawLoad {
        source: "app".to_owned(),
        table: DeclaredTable {
            name: "orders".to_owned(),
            key: vec!["id".to_owned()],
            incremental: None,
        },
        load_id: format!("load-{}", rows.len()),
        extracted_at: Timestamp::from_unix_nanos(0),
        rows: vec![RecordBatch::try_new(Arc::clone(&schema), vec![ids, countries]).unwrap()],
        schema,
        deleted: Vec::new(),
    }
}

/// Writes the seed `countries` to `raw.seed__countries`, as a seed load would.
#[cfg(test)]
fn write_seed(raw: &mut DuckLakeAdapter) {
    let schema = Arc::new(Schema::new(vec![
        Field::new("code", DataType::Utf8, true),
        Field::new("name", DataType::Utf8, true),
    ]));
    let codes: ArrayRef = Arc::new(StringArray::from(vec!["PE", "CL"]));
    let names: ArrayRef = Arc::new(StringArray::from(vec!["Peru", "Chile"]));
    let batch = RecordBatch::try_new(Arc::clone(&schema), vec![codes, names]).unwrap();
    let stream = arrow::record_batch::RecordBatchIterator::new([Ok(batch)], schema);
    raw.write("seed__countries", WriteMode::Replace, Box::new(stream))
        .unwrap();
    raw.commit().unwrap();
}

/// The local raw zone with its catalog and data under `dir`, outside the project.
#[cfg(test)]
fn raw_storage(dir: &Path) -> ZoneStorage {
    let data = dir.join("data").display().to_string();
    ZoneStorage {
        zone: Zone::Raw,
        profile: Profile::Local,
        location: data.clone(),
        catalog: Catalog::DuckDb {
            file: dir.join("catalog.ducklake").display().to_string(),
        },
        data_path: data,
        clients: Clients::OneProcess,
    }
}

/// A new, empty directory under the target directory of the tests.
#[cfg(test)]
fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "mergus-raw-inputs-{:016x}",
        RealRandom::new().next_u64()
    ));
    RealFileSystem.create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
fn clock() -> Arc<dyn mergus_ports::Clock> {
    Arc::new(RealClock::new())
}

/// `error` as the failure of taking the snapshot.
#[cfg(test)]
fn failure(error: RawError) -> StepFailure {
    StepFailure {
        name: "raw".to_owned(),
        code: error.code(),
        message: error.to_string(),
    }
}

/// Whether `error` says an extension is not cached and cannot be downloaded.
#[cfg(test)]
fn extension_unavailable(error: &RawError) -> bool {
    let RawError::Storage(AdapterError::Storage(inner)) = error else {
        return false;
    };
    matches!(
        inner.downcast_ref::<DuckLakeError>(),
        Some(DuckLakeError::Port(PortError::ExtensionUnavailable { .. }))
    )
}
