//! Tests of the real `Database` (MG-REQ-0194, MG-REQ-0196). They never use a database server.
//! A missing extension is installed from an unreachable repository, and a download is covered
//! by a local repository holding the extension of the user cache. Only in CI (the variable
//! `CI` set) is an extension missing from the user cache first installed there from
//! `extensions.duckdb.org`; elsewhere those parts are skipped.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use std::time::Duration;

use super::RealDatabase;
use crate::real::RealEnvironment;
use crate::{
    Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Environment, Extension,
    LoadedExtension, PortError, PostgresSource, SqlValue,
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

/// The file name DuckDB caches `extension` under.
fn file_name(extension: Extension) -> &'static str {
    match extension {
        Extension::DuckLake => "ducklake.duckdb_extension",
        Extension::Postgres => "postgres_scanner.duckdb_extension",
        Extension::Httpfs => "httpfs.duckdb_extension",
    }
}

/// Where DuckDB keeps `extension` under the extension directory `dir`.
fn cached_extension(
    connection: &mut dyn DatabaseConnection,
    dir: &Path,
    extension: Extension,
) -> PathBuf {
    let version = text(connection, "SELECT library_version FROM pragma_version()");
    let platform = text(connection, "SELECT platform FROM pragma_platform()");
    dir.join(version).join(platform).join(file_name(extension))
}

/// Returns a user cache that got `extension` from a local repository holding the signed
/// extension of the real user cache, and the path of the extension under it. When the real
/// user cache does not hold it, CI (the variable `CI` set) first installs it there from
/// `extensions.duckdb.org`, so a failed install fails the test; elsewhere it returns `None`.
fn downloaded(extension: Extension) -> Option<(PathBuf, PathBuf)> {
    let real = RealDatabase::new(&RealEnvironment.cache_dir()?);
    let mut probe = real.connect(&DatabaseLocation::InMemory).unwrap();
    let cached = cached_extension(probe.as_mut(), real.extension_dir(), extension);
    if !cached.is_file() {
        // Outside CI, skip.
        std::env::var_os("CI")?;
        probe.load_extension(extension).unwrap();
        assert!(cached.is_file(), "{}", cached.display());
    }
    let repository = scratch("repository");
    let relative = cached.strip_prefix(real.extension_dir()).unwrap();
    fs::create_dir_all(repository.join(relative).parent().unwrap()).unwrap();
    fs::copy(&cached, repository.join(relative)).unwrap();
    let user_cache = scratch("download");
    let downloading = database(&user_cache, &repository.display().to_string());
    let mut connection = downloading.connect(&DatabaseLocation::InMemory).unwrap();
    connection.load_extension(extension).unwrap();
    let path = downloading.extension_dir().join(relative);
    assert!(path.is_file());
    Some((user_cache, path))
}

/// Checks that the registry of `connection` records the version DuckDB reports for
/// `extension`, and only it.
fn assert_recorded(connection: &mut dyn DatabaseConnection, extension: Extension) {
    let sql = "SELECT extension_version FROM duckdb_extensions() \
               WHERE extension_name = ? OR list_contains(aliases, ?)";
    let name = SqlValue::Text(extension.name().to_owned());
    let version = text_with(connection, sql, &[name.clone(), name]);
    assert!(!version.is_empty());
    let recorded = LoadedExtension { extension, version };
    assert_eq!(connection.loaded_extensions(), vec![recorded]);
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

/// MG-REQ-0194.3: `ducklake` and `postgres`, missing and unreachable, fail with a typed error
/// naming the host and the cache directory `dir`.
fn missing_and_unreachable(connection: &mut dyn DatabaseConnection, user_cache: &Path, dir: &Path) {
    let catalog = DuckLakeCatalog {
        alias: "lake".to_owned(),
        metadata: user_cache.join("metadata.ducklake").display().to_string(),
        data_path: user_cache.join("data").display().to_string(),
    };
    let ducklake = connection.attach_ducklake(&catalog).unwrap_err();
    let postgres = connection
        .attach_postgres(&crm("host=db.invalid"))
        .unwrap_err();
    for (error, extension) in [(ducklake, "ducklake"), (postgres, "postgres")] {
        assert_eq!(
            error,
            PortError::ExtensionUnavailable {
                extension: extension.to_owned(),
                host: "127.0.0.1:1".to_owned(),
                cache_dir: dir.to_path_buf(),
            }
        );
        let message = error.to_string();
        assert!(message.contains("127.0.0.1:1") && message.contains(&dir.display().to_string()));
    }
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

    missing_and_unreachable(connection.as_mut(), &user_cache, &dir);
    assert!(connection.loaded_extensions().is_empty());
    // The real repository is DuckDB's own.
    assert_eq!(
        RealDatabase::new(&user_cache).host(),
        "extensions.duckdb.org"
    );

    // The rest needs the signed extensions, already in the user cache.
    download_then_cache_hit();
    postgres_download_then_cache_hit();
}

/// MG-REQ-0194.1 and MG-REQ-0194.2, when the signed extension is already in the user cache:
/// the first use installs it from the repository into the cache, later uses load it from
/// there with the repository unreachable, and a DuckLake catalog is attached and used.
fn download_then_cache_hit() {
    // Download: the first use installs it from the repository into the cache.
    let Some((user_cache, _)) = downloaded(Extension::DuckLake) else {
        return;
    };

    // Cache hit: later uses load it from the cache, with the repository unreachable, and
    // record the version DuckDB reports.
    let offline = database(&user_cache, UNREACHABLE);
    let mut connection = offline.connect(&DatabaseLocation::InMemory).unwrap();
    attach_and_use(connection.as_mut(), &scratch("lake"));
    assert_recorded(connection.as_mut(), Extension::DuckLake);
}

/// MG-REQ-0194.1 and MG-REQ-0194.2 for `postgres`, when the signed extension is already in
/// the user cache: installed into the cache on first use, then loaded from there offline,
/// with its version recorded.
fn postgres_download_then_cache_hit() {
    let Some((user_cache, _)) = downloaded(Extension::Postgres) else {
        return;
    };
    let offline = database(&user_cache, UNREACHABLE);
    let mut connection = offline.connect(&DatabaseLocation::InMemory).unwrap();
    connection.load_extension(Extension::Postgres).unwrap();
    assert_recorded(connection.as_mut(), Extension::Postgres);
}

/// A PostgreSQL source named `crm` reached through `connection`.
fn crm(connection: &str) -> PostgresSource {
    PostgresSource {
        alias: "crm".to_owned(),
        connection: connection.to_owned(),
        max_connections: 3,
        query_timeout: Duration::from_secs(90),
    }
}

#[test]
fn mg_req_0196_database_sources_read_by_duckdb() {
    // MG-REQ-0196.1: a DuckDB file is attached read-only with no extension, and read as Arrow.
    let dir = scratch("source");
    let file = dir.join("sales.duckdb");
    let writer = RealDatabase::new(&dir);
    let mut connection = writer
        .connect(&DatabaseLocation::File(file.clone()))
        .unwrap();
    connection
        .execute("CREATE TABLE orders (id BIGINT)", &[])
        .unwrap();
    connection
        .execute("INSERT INTO orders VALUES (?)", &[SqlValue::Integer(42)])
        .unwrap();
    drop(connection);
    let reader = database(&dir, UNREACHABLE);
    let mut connection = reader.connect(&DatabaseLocation::InMemory).unwrap();
    connection.attach_duckdb_file(&file).unwrap();
    let id = text(connection.as_mut(), "SELECT id::VARCHAR FROM sales.orders");
    assert_eq!(id, "42");
    let insert = "INSERT INTO sales.orders VALUES (1)";
    assert!(connection.execute(insert, &[]).is_err());
    assert!(connection.loaded_extensions().is_empty());
    connection.detach("sales").unwrap();
    assert!(
        connection
            .query_arrow("SELECT id FROM sales.orders", &[])
            .is_err()
    );

    // The rest needs the signed `postgres` extension, already in the user cache.
    postgres_without_server();
}

/// MG-REQ-0196.2 and MG-REQ-0196.3 on the real DuckDB with no server: the attach fails on a
/// socket directory that does not exist, after the pool and timeout settings were set; the
/// error never holds the connection value and no secret is left behind.
fn postgres_without_server() {
    let Some((user_cache, _)) = downloaded(Extension::Postgres) else {
        return;
    };
    let offline = database(&user_cache, UNREACHABLE);
    let mut connection = offline.connect(&DatabaseLocation::InMemory).unwrap();
    // The error is the attach's failure to reach the server, so the settings and the secret,
    // with the session parameters of MG-REQ-0429.2 in either form, were accepted with their
    // bound values.
    for value in [
        "host=/nonexistent/mergus dbname=crm password=hunter2",
        "postgresql://reader:hunter2@/crm?host=/nonexistent/mergus",
    ] {
        let error = connection.attach_postgres(&crm(value)).unwrap_err();
        let message = error.to_string();
        assert!(matches!(error, PortError::Failed(_)), "{error:?}");
        assert!(
            message.contains("IO Error: Unable to connect to Postgres"),
            "{message}"
        );
        assert!(!message.contains("hunter2"), "{message}");
    }
    for (setting, expected) in [
        ("pg_pool_max_connections", "3"),
        ("pg_pool_acquire_mode", "wait"),
        ("pg_statement_timeout_millis", "90000"),
    ] {
        let sql = "SELECT current_setting(?)::VARCHAR";
        let params = [SqlValue::Text(setting.to_owned())];
        assert_eq!(text_with(connection.as_mut(), sql, &params), expected);
    }
    let secrets = "SELECT count(*)::VARCHAR FROM duckdb_secrets()";
    assert_eq!(text(connection.as_mut(), secrets), "0");
    assert_recorded(connection.as_mut(), Extension::Postgres);
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

#[test]
fn real_database_writes_parquet_with_no_network() {
    // Parquet is linked into the bundled DuckDB: no extension is installed, from a repository
    // that cannot be reached, nor loaded.
    let dir = scratch("parquet");
    let file = dir.join("x.parquet");
    let offline = database(&dir, UNREACHABLE);
    let mut connection = offline.connect(&DatabaseLocation::InMemory).unwrap();
    let copy = "COPY (SELECT 42 AS id) TO ? (FORMAT parquet)";
    let path = SqlValue::Text(file.display().to_string());
    connection
        .execute(copy, std::slice::from_ref(&path))
        .unwrap();
    assert!(file.is_file());
    let read = "SELECT id::VARCHAR FROM read_parquet(?)";
    assert_eq!(text_with(connection.as_mut(), read, &[path]), "42");
    assert!(connection.loaded_extensions().is_empty());
}
