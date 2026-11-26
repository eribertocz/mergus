//! Tests of the encryption of raw (MG-REQ-0515), on the simulated `Database` and on the real
//! DuckDB. The real part returns at once when an extension is not cached and cannot be
//! downloaded, except in CI (`CI` set), where that fails the test.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::{ArrayRef, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use mergus_adapter::{AdapterError, StorageAdapter};
use mergus_adapter_ducklake::{DuckLakeAdapter, DuckLakeError};
use mergus_ports::real::{RealClock, RealDatabase, RealEnvironment, RealFileSystem, RealRandom};
use mergus_ports::{
    Clock, Database, DatabaseLocation, DuckLakeCatalog, Environment, FileSystem, PortError, Random,
    Timestamp,
};
use mergus_runtime::{
    Catalog, Clients, Profile, RawError, RawLoad, Zone, ZoneStorage, open_raw, write_raw,
};
use mergus_sim::{ClockFaults, DatabaseFaults, SimClock, SimDatabase};
use mergus_source::DeclaredTable;

/// A text that must never appear in a data file of an encrypted catalog.
const SENTINEL: &str = "mergus-plaintext-sentinel";

#[cfg(test)]
fn sim_clock() -> Arc<dyn Clock> {
    Arc::new(SimClock::new(
        1,
        Timestamp::from_unix_nanos(0),
        ClockFaults::default(),
    ))
}

#[cfg(test)]
/// The raw zone of `profile`, its catalog `catalog` and its data under `data_path`.
fn raw(profile: Profile, catalog: Catalog, data_path: &str) -> ZoneStorage {
    ZoneStorage {
        zone: Zone::Raw,
        profile,
        location: data_path.to_owned(),
        catalog,
        data_path: data_path.to_owned(),
        clients: Clients::OneProcess,
    }
}

#[cfg(test)]
fn duckdb_file(file: &str) -> Catalog {
    Catalog::DuckDb {
        file: file.to_owned(),
    }
}

#[test]
fn mg_req_0515_raw_encrypted_at_rest() {
    let root = Path::new("/project");

    // MG-REQ-0515.1: raw is attached with the `ENCRYPTED` option only, as `raw`.
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let storage = raw(
        Profile::Local,
        duckdb_file("/srv/raw/catalog.ducklake"),
        "/srv/raw/data/",
    );
    open_raw(&database, &storage, root, None, sim_clock()).unwrap();
    let expected = DuckLakeCatalog {
        alias: "raw".to_owned(),
        metadata: "/srv/raw/catalog.ducklake".to_owned(),
        data_path: "/srv/raw/data/".to_owned(),
    };
    assert_eq!(database.attached_encrypted(), [expected]);
    assert!(database.attached().is_empty());

    unencrypted_is_refused(root, &storage);
    inside_the_project_is_refused(root);
    real_catalog_is_encrypted();
}

/// MG-REQ-0515.3: an existing catalog that is not encrypted is refused, by name; a
/// PostgreSQL catalog by its secret reference, never by its connection value.
#[cfg(test)]
fn unencrypted_is_refused(root: &Path, storage: &ZoneStorage) {
    let database = SimDatabase::new(1, DatabaseFaults::default());
    database.record_unencrypted("/srv/raw/catalog.ducklake");
    let error = open_raw(&database, storage, root, None, sim_clock())
        .err()
        .unwrap();
    assert_eq!(error.code(), Some("MG-0503"));
    let RawError::UnencryptedCatalog { catalog } = &error else {
        panic!("{error}");
    };
    assert_eq!(catalog, "/srv/raw/catalog.ducklake");
    assert!(
        error.to_string().contains("/srv/raw/catalog.ducklake"),
        "{error}"
    );

    let database = SimDatabase::new(1, DatabaseFaults::default());
    database.record_unencrypted("postgres:password=hunter2");
    let cloud = raw(
        Profile::Cloud,
        Catalog::Postgres {
            secret: "secret:raw_catalog".to_owned(),
        },
        "s3://raw/",
    );
    let error = open_raw(
        &database,
        &cloud,
        root,
        Some("password=hunter2"),
        sim_clock(),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("secret:raw_catalog"), "{error}");
    assert!(!error.to_string().contains("hunter2"), "{error}");
}

#[cfg(test)]
/// MG-REQ-0515.2: a `local` or `shared` catalog inside the project root is refused before
/// anything is attached; one that only looks like it is inside is not.
fn inside_the_project_is_refused(root: &Path) {
    let inside = [
        (Profile::Local, duckdb_file("lake/raw/catalog.ducklake")),
        (
            Profile::Local,
            duckdb_file("/project/./lake/../catalog.ducklake"),
        ),
        (
            Profile::Shared,
            Catalog::Sqlite {
                file: "/project/raw.sqlite".to_owned(),
            },
        ),
    ];
    for (profile, catalog) in inside {
        let database = SimDatabase::new(1, DatabaseFaults::default());
        let storage = raw(profile, catalog.clone(), "lake/raw/data/");
        let error = open_raw(&database, &storage, root, None, sim_clock())
            .err()
            .unwrap();
        assert_eq!(error.code(), Some("MG-0502"), "{catalog:?}");
        let RawError::CatalogInsideProject { path, .. } = &error else {
            panic!("{error}");
        };
        let (Catalog::DuckDb { file } | Catalog::Sqlite { file }) = &catalog else {
            panic!("a file catalog");
        };
        assert_eq!(path, file);
        let message = error.to_string();
        assert!(
            message.contains(file.as_str()) && message.contains("outside"),
            "{message}"
        );
        assert!(database.attached_encrypted().is_empty());
    }
    let database = SimDatabase::new(1, DatabaseFaults::default());
    let outside = raw(
        Profile::Local,
        duckdb_file("/project/../project-raw/catalog.ducklake"),
        "lake/raw/data/",
    );
    open_raw(&database, &outside, root, None, sim_clock()).unwrap();
    assert_eq!(database.attached_encrypted().len(), 1);
}

#[cfg(test)]
/// A new, empty directory under the target directory of the tests.
fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("mergus-raw-{:016x}", RealRandom::new().next_u64()));
    RealFileSystem.create_dir_all(&dir).unwrap();
    dir
}

#[cfg(test)]
/// Whether `error` says an extension is not cached and cannot be downloaded.
fn extension_unavailable(error: &RawError) -> bool {
    let RawError::Storage(AdapterError::Storage(inner)) = error else {
        return false;
    };
    matches!(
        inner.downcast_ref::<DuckLakeError>(),
        Some(DuckLakeError::Port(PortError::ExtensionUnavailable { .. }))
    )
}

#[cfg(test)]
/// Load `load` of `notes`, keyed by `id`: the note `load`, holding [`SENTINEL`].
fn notes(load: i64) -> RawLoad {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("text", DataType::Utf8, false),
    ]));
    let ids: ArrayRef = Arc::new(Int64Array::from(vec![load]));
    let texts: ArrayRef = Arc::new(StringArray::from(vec![SENTINEL]));
    RawLoad {
        source: "app".to_owned(),
        table: DeclaredTable {
            name: "notes".to_owned(),
            key: vec!["id".to_owned()],
            incremental: None,
        },
        load_id: format!("load-{load}"),
        extracted_at: Timestamp::from_unix_nanos(0),
        rows: vec![RecordBatch::try_new(Arc::clone(&schema), vec![ids, texts]).unwrap()],
        schema,
        deleted: Vec::new(),
    }
}

#[cfg(test)]
/// The paths of every file under `dir`.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in RealFileSystem.list_dir(dir).unwrap() {
        match RealFileSystem.list_dir(&entry) {
            Ok(_) => found.extend(files(&entry)),
            Err(_) => found.push(entry),
        }
    }
    found
}

#[cfg(test)]
/// MG-REQ-0515.1 and MG-REQ-0515.3 on the real DuckDB: the data files of raw hold no
/// plaintext and the catalog keeps a key per file; an unencrypted catalog is refused.
fn real_catalog_is_encrypted() {
    let in_ci = RealEnvironment.var("CI").is_some();
    let Some(cache_dir) = RealEnvironment.cache_dir() else {
        assert!(!in_ci, "no user cache directory in CI");
        return;
    };
    let database = RealDatabase::new(&cache_dir);
    let dir = scratch();
    RealFileSystem.create_dir_all(&dir.join("raw")).unwrap();
    let file = dir
        .join("raw")
        .join("catalog.ducklake")
        .display()
        .to_string();
    let data = dir.join("raw").join("data").display().to_string();
    let storage = raw(Profile::Local, duckdb_file(&file), &data);
    let root = dir.join("project");
    let mut adapter: DuckLakeAdapter =
        match open_raw(&database, &storage, &root, None, Arc::new(RealClock::new())) {
            Ok(adapter) => adapter,
            Err(error) if extension_unavailable(&error) => {
                assert!(!in_ci, "an extension is unavailable in CI: {error}");
                return;
            }
            Err(error) => panic!("{error}"),
        };
    let first = write_raw(&mut adapter, &notes(1), None).unwrap();
    // The second load reads the current rows back from the encrypted files.
    let second = write_raw(&mut adapter, &notes(2), Some(&first)).unwrap();
    let current: usize = adapter
        .read_at("app__notes", &second)
        .unwrap()
        .map(|batch| batch.unwrap().num_rows())
        .sum();
    assert_eq!(current, 2);
    drop(adapter);
    no_plaintext_and_a_key_per_file(&database, &file, &data);
    plain_catalog_is_refused(&database, &dir, &root);
}

/// The data files under `data` hold no plaintext, and the catalog `file` keeps a key for
/// each of them.
#[cfg(test)]
fn no_plaintext_and_a_key_per_file(database: &RealDatabase, file: &str, data: &str) {
    let written = files(Path::new(data));
    assert!(!written.is_empty());
    for path in &written {
        let bytes = RealFileSystem.read(path).unwrap();
        let plain = bytes
            .windows(SENTINEL.len())
            .any(|window| window == SENTINEL.as_bytes());
        assert!(!plain, "{} holds plaintext", path.display());
    }
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    connection.attach_duckdb_file(Path::new(file)).unwrap();
    let keys = "SELECT count(*) AS files FROM catalog.ducklake_data_file \
                WHERE encryption_key IS NOT NULL AND encryption_key <> ''";
    let unkeyed = "SELECT count(*) AS files FROM catalog.ducklake_data_file \
                   WHERE encryption_key IS NULL OR encryption_key = ''";
    // Every write of each table is a file of its own, and none is inlined in the catalog.
    assert!(count(connection.as_mut(), keys) >= 4);
    assert_eq!(count(connection.as_mut(), unkeyed), 0);
}

/// A catalog created without encryption is refused.
#[cfg(test)]
fn plain_catalog_is_refused(database: &RealDatabase, dir: &Path, root: &Path) {
    let plain = dir.join("plain.ducklake").display().to_string();
    let catalog = DuckLakeCatalog {
        alias: "plain".to_owned(),
        metadata: plain.clone(),
        data_path: dir.join("plain").display().to_string(),
    };
    let mut connection = database.connect(&DatabaseLocation::InMemory).unwrap();
    connection.attach_ducklake(&catalog).unwrap();
    drop(connection);
    let storage = raw(Profile::Local, duckdb_file(&plain), &catalog.data_path);
    let error = open_raw(database, &storage, root, None, Arc::new(RealClock::new()))
        .err()
        .unwrap();
    let RawError::UnencryptedCatalog { catalog } = &error else {
        panic!("{error}");
    };
    assert_eq!(catalog, &plain);
}

#[cfg(test)]
/// The one `BIGINT` that `sql` returns.
fn count(connection: &mut dyn mergus_ports::DatabaseConnection, sql: &str) -> i64 {
    use arrow::array::AsArray;
    let batches = connection.query_arrow(sql, &[]).unwrap();
    batches[0]
        .column(0)
        .as_primitive::<arrow::datatypes::Int64Type>()
        .value(0)
}
