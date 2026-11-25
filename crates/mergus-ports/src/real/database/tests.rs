//! Tests of the real `Database` (MG-REQ-0194). They never use the network: a missing
//! extension is installed from an unreachable repository, and a download is covered by a
//! local repository holding the extension of the user cache, when it is already cached.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{EXTENSION, RealDatabase};
use crate::real::RealEnvironment;
use crate::{
    Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Environment, PortError,
    SqlValue,
};

/// A repository no extension can be downloaded from: nothing listens on port 1.
const UNREACHABLE: &str = "http://127.0.0.1:1";

/// Returns a new, empty directory for one test.
fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mergus-db-{}-{name}-{id}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A database caching extensions under `user_cache` and installing them from `repository`.
fn database(user_cache: &Path, repository: &str) -> RealDatabase {
    let mut database = RealDatabase::new(user_cache);
    database.repository = Some(repository.to_owned());
    database
}

/// Returns the one text value of `sql` with `params`.
fn text_with(connection: &mut dyn DatabaseConnection, sql: &str, params: &[SqlValue]) -> String {
    let batches = connection.query_arrow(sql, params).unwrap();
    let column = batches[0].column(0);
    let column = arrow::array::cast::as_string_array(column);
    column.value(0).to_owned()
}

/// Returns the one text value of `sql`.
fn text(connection: &mut dyn DatabaseConnection, sql: &str) -> String {
    text_with(connection, sql, &[])
}

/// Where DuckDB keeps `ducklake` under the extension directory `dir`.
fn cached_extension(connection: &mut dyn DatabaseConnection, dir: &Path) -> PathBuf {
    let version = text(connection, "SELECT library_version FROM pragma_version()");
    let platform = text(connection, "SELECT platform FROM pragma_platform()");
    dir.join(version)
        .join(platform)
        .join(format!("{EXTENSION}.duckdb_extension"))
}

/// Attaches a new DuckLake catalog under `dir` and writes and reads one row through it.
fn attach_and_use(connection: &mut dyn DatabaseConnection, dir: &Path) {
    let catalog = DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: dir.join("metadata.ducklake").display().to_string(),
        data_path: dir.join("data").display().to_string(),
    };
    connection.attach_ducklake(&catalog).unwrap();
    connection
        .execute("CREATE TABLE lake.items (id BIGINT)", &[])
        .unwrap();
    let inserted = connection
        .execute("INSERT INTO lake.items VALUES (?)", &[SqlValue::Integer(7)])
        .unwrap();
    assert_eq!(inserted, 1);
    assert_eq!(text(connection, "SELECT id::VARCHAR FROM lake.items"), "7");
}

#[test]
fn mg_req_0194_ducklake_extension_pinned_and_cached() {
    // The extension directory is under the user cache; automatic installing and loading are
    // off and unsigned extensions are refused.
    let user_cache = scratch("missing");
    let missing = database(&user_cache, UNREACHABLE);
    let dir = user_cache.join("mergus").join("duckdb").join("extensions");
    assert_eq!(missing.extension_dir(), dir);
    let mut connection = missing.connect(&DatabaseLocation::InMemory).unwrap();
    for setting in [
        "autoinstall_known_extensions",
        "autoload_known_extensions",
        "allow_unsigned_extensions",
    ] {
        let sql = "SELECT current_setting(?)::VARCHAR";
        let value = text_with(
            connection.as_mut(),
            sql,
            &[SqlValue::Text(setting.to_owned())],
        );
        assert_eq!(value, "false", "{setting}");
    }

    // Missing and unreachable: a typed error naming the host and the cache directory.
    let catalog = DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: user_cache.join("metadata.ducklake").display().to_string(),
        data_path: user_cache.join("data").display().to_string(),
    };
    let error = connection.attach_ducklake(&catalog).unwrap_err();
    assert_eq!(
        error,
        PortError::ExtensionUnavailable {
            extension: EXTENSION.to_owned(),
            host: "127.0.0.1:1".to_owned(),
            cache_dir: dir.clone(),
        }
    );
    let message = error.to_string();
    assert!(message.contains("127.0.0.1:1") && message.contains(&dir.display().to_string()));
    // The real repository is DuckDB's own.
    assert_eq!(
        RealDatabase::new(&user_cache).host(),
        "extensions.duckdb.org"
    );

    // The rest needs the signed extension, already in the user cache.
    download_then_cache_hit();
}

/// MG-REQ-0194.1 and MG-REQ-0194.2, when the signed extension is already in the user cache:
/// the first use installs it from the repository into the cache, later uses load it from
/// there with the repository unreachable, and a DuckLake catalog is attached and used.
fn download_then_cache_hit() {
    let Some(real_cache) = RealEnvironment.cache_dir() else {
        return;
    };
    let real = RealDatabase::new(&real_cache);
    let mut probe = real.connect(&DatabaseLocation::InMemory).unwrap();
    let cached = cached_extension(probe.as_mut(), real.extension_dir());
    if !cached.is_file() {
        return;
    }

    // Download: the first use installs it from the repository into the cache.
    let repository = scratch("repository");
    let relative = cached.strip_prefix(real.extension_dir()).unwrap();
    fs::create_dir_all(repository.join(relative).parent().unwrap()).unwrap();
    fs::copy(&cached, repository.join(relative)).unwrap();
    let user_cache = scratch("download");
    let downloading = database(&user_cache, &repository.display().to_string());
    let mut connection = downloading.connect(&DatabaseLocation::InMemory).unwrap();
    attach_and_use(connection.as_mut(), &scratch("lake"));
    assert!(downloading.extension_dir().join(relative).is_file());

    // Cache hit: later uses load it from the cache, with the repository unreachable.
    let offline = database(&user_cache, UNREACHABLE);
    let mut connection = offline.connect(&DatabaseLocation::InMemory).unwrap();
    attach_and_use(connection.as_mut(), &scratch("lake"));
}

#[test]
fn real_database_runs_sql_and_reads_and_writes_arrow() {
    let dir = scratch("arrow");
    let database = RealDatabase::new(&dir);
    let location = DatabaseLocation::File(dir.join("main.duckdb"));
    let mut connection = database.connect(&location).unwrap();
    connection
        .execute("CREATE TABLE items (id BIGINT, name VARCHAR)", &[])
        .unwrap();
    let params = [SqlValue::Integer(1), SqlValue::Text("a'b".to_owned())];
    let inserted = connection
        .execute("INSERT INTO items VALUES (?, ?)", &params)
        .unwrap();
    assert_eq!(inserted, 1);
    let batches = connection
        .query_arrow("SELECT * FROM items WHERE id > ?", &[SqlValue::Integer(0)])
        .unwrap();
    connection.append_arrow("items", &batches[0]).unwrap();
    let count = "SELECT count(*)::VARCHAR FROM items WHERE name = ?";
    let text_param = [SqlValue::Text("a'b".to_owned())];
    assert_eq!(text_with(connection.as_mut(), count, &text_param), "2");
}
