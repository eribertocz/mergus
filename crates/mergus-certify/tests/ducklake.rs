//! MG-REQ-0289: the conformance kit runs on the DuckLake adapter over a real DuckLake catalog.
//! Crash points are the simulated `Database`'s error faults: once the kit crashes the client,
//! every later call of its connections goes to a `SimDatabase` that fails every call.
//!
//! The real `Database` installs the signed `ducklake` extension into the user cache the first
//! time. Outside CI, the test returns at once when the extension is not cached and cannot be
//! downloaded; in CI (`CI` set) that fails the test, so a green CI proves the kit ran.

use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::RecordBatch;
use mergus_adapter::{AdapterError, StorageAdapter};
use mergus_adapter_ducklake::{DuckLakeAdapter, DuckLakeError};
use mergus_certify::{Faults, Harness, certify};
use mergus_ports::real::{RealClock, RealDatabase, RealEnvironment, RealFileSystem, RealRandom};
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Environment, Extension,
    FileSystem, LoadedExtension, PortError, Random, SqlValue,
};
use mergus_sim::{DatabaseFaults, Rate, SimDatabase};

/// The real `Database`, whose connections crash when the kit's `Faults` say so.
#[cfg(test)]
struct Crashing {
    real: RealDatabase,
    faults: Arc<Faults>,
}

#[cfg(test)]
impl Database for Crashing {
    fn connect(
        &self,
        location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        let failing = SimDatabase::new(
            0,
            DatabaseFaults {
                error: Rate::ALWAYS,
            },
        );
        Ok(Box::new(CrashingConnection {
            real: self.real.connect(location)?,
            failing: failing.connect(location)?,
            faults: Arc::clone(&self.faults),
        }))
    }
}

/// A real connection until the kit crashes the client, then a connection that always fails.
#[cfg(test)]
struct CrashingConnection {
    real: Box<dyn DatabaseConnection>,
    failing: Box<dyn DatabaseConnection>,
    faults: Arc<Faults>,
}

#[cfg(test)]
impl CrashingConnection {
    fn target(&mut self) -> &mut Box<dyn DatabaseConnection> {
        if self.faults.crashed() {
            &mut self.failing
        } else {
            &mut self.real
        }
    }
}

#[cfg(test)]
impl DatabaseConnection for CrashingConnection {
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError> {
        self.target().execute(sql, params)
    }

    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError> {
        self.target().query_arrow(sql, params)
    }

    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError> {
        self.target().append_arrow(table, batch)
    }

    fn attach_ducklake(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.target().attach_ducklake(catalog)
    }

    fn load_extension(&mut self, extension: Extension) -> Result<(), PortError> {
        self.target().load_extension(extension)
    }

    fn loaded_extensions(&self) -> Vec<LoadedExtension> {
        self.real.loaded_extensions()
    }
}

/// Each store is a new DuckLake catalog in a new directory under the tests' target directory.
#[cfg(test)]
struct DuckLake {
    cache_dir: PathBuf,
}

#[cfg(test)]
impl Harness for DuckLake {
    type Store = DuckLakeCatalog;

    fn store(&self) -> Result<DuckLakeCatalog, AdapterError> {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "mergus-certify-{:016x}",
            RealRandom::new().next_u64()
        ));
        RealFileSystem
            .create_dir_all(&dir)
            .map_err(|error| AdapterError::Storage(Box::new(error)))?;
        Ok(DuckLakeCatalog {
            alias: "lake".to_owned(),
            metadata: dir.join("metadata.ducklake").display().to_string(),
            data_path: dir.join("data").display().to_string(),
        })
    }

    fn open(
        &self,
        store: &DuckLakeCatalog,
        faults: Arc<Faults>,
    ) -> Result<Box<dyn StorageAdapter>, AdapterError> {
        let database = Crashing {
            real: RealDatabase::new(&self.cache_dir),
            faults,
        };
        let adapter = DuckLakeAdapter::open(&database, store, Arc::new(RealClock::new()))?;
        Ok(Box::new(adapter))
    }
}

/// Returns whether `error` says the `ducklake` extension is not cached and cannot be
/// downloaded.
#[cfg(test)]
fn extension_unavailable(error: &AdapterError) -> bool {
    let AdapterError::Storage(error) = error else {
        return false;
    };
    matches!(
        error.downcast_ref::<DuckLakeError>(),
        Some(DuckLakeError::Port(PortError::ExtensionUnavailable { .. }))
    )
}

#[test]
fn mg_req_0289_certify_kit_runs_on_ducklake() {
    let in_ci = RealEnvironment.var("CI").is_some();
    let Some(cache_dir) = RealEnvironment.cache_dir() else {
        assert!(!in_ci, "no user cache directory in CI");
        return;
    };
    let harness = DuckLake { cache_dir };
    let store = harness.store().unwrap();
    if let Err(error) = harness.open(&store, Faults::new()) {
        assert!(
            extension_unavailable(&error) && !in_ci,
            "the DuckLake adapter does not open: {error}"
        );
        return;
    }
    let report = certify(&harness);
    assert!(report.passed(), "{report}");
    // Every check of the kit ran on DuckLake, except those its declaration rules out.
    assert!(!report.outcomes().is_empty(), "{report}");
}
