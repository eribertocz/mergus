//! A simulated DuckDB: results recorded from the real one, with errors (MG-REQ-0194,
//! MG-REQ-0196).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use arrow::record_batch::RecordBatch;
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Extension, LoadedExtension,
    PortError, SqlValue,
};

use crate::rng::{Rate, SimRng, lock};

/// The database faults to inject, drawn before each call of a connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DatabaseFaults {
    /// How often a call fails.
    pub error: Rate,
}

/// What the real database answered to one statement.
#[derive(Clone, Debug)]
enum Answer {
    /// The number of rows a statement changed.
    Changed(usize),
    /// The rows of a query.
    Rows(Vec<RecordBatch>),
}

#[derive(Debug)]
struct Shared {
    faults: DatabaseFaults,
    rng: Mutex<SimRng>,
    answers: Mutex<Vec<(String, Vec<SqlValue>, Answer)>>,
    appended: Mutex<BTreeMap<String, Vec<RecordBatch>>>,
    attached: Mutex<Vec<DuckLakeCatalog>>,
    encrypted: Mutex<Vec<DuckLakeCatalog>>,
    unencrypted: Mutex<Vec<String>>,
    extensions: Mutex<BTreeMap<Extension, String>>,
    executed: Mutex<Vec<(String, Vec<SqlValue>)>>,
}

impl Shared {
    /// Draws the error fault of one call on `what`.
    fn enter(&self, what: &str) -> Result<(), PortError> {
        if lock(&self.rng).hits(self.faults.error) {
            return Err(PortError::Failed(format!(
                "simulated database error on {what}"
            )));
        }
        Ok(())
    }

    /// Returns the answer recorded for `sql` with `params`.
    fn answer(&self, sql: &str, params: &[SqlValue]) -> Result<Answer, PortError> {
        self.enter(sql)?;
        lock(&self.answers)
            .iter()
            .find(|(recorded, bound, _)| recorded == sql && bound.as_slice() == params)
            .map(|(_, _, answer)| answer.clone())
            .ok_or_else(|| PortError::NotFound(format!("a recorded result for `{sql}`")))
    }
}

/// Answers each statement with the result recorded for its SQL text and parameters, and
/// fails with [`PortError::NotFound`] for a statement that has none. Appended batches and
/// attached catalogs are kept for the test to inspect, as is every statement run with
/// [`DatabaseConnection::execute`]. An extension loads only when its version was recorded.
/// Every connection shares them.
#[derive(Clone, Debug)]
pub struct SimDatabase {
    shared: Arc<Shared>,
}

impl SimDatabase {
    /// Returns a database with no recorded result, drawing its faults from `seed`.
    #[must_use]
    pub fn new(seed: u64, faults: DatabaseFaults) -> Self {
        Self {
            shared: Arc::new(Shared {
                faults,
                rng: Mutex::new(SimRng::new(seed)),
                answers: Mutex::new(Vec::new()),
                appended: Mutex::new(BTreeMap::new()),
                attached: Mutex::new(Vec::new()),
                encrypted: Mutex::new(Vec::new()),
                unencrypted: Mutex::new(Vec::new()),
                extensions: Mutex::new(BTreeMap::new()),
                executed: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Records that the real database changed `changed` rows for `sql` with `params`.
    pub fn record_execute(&self, sql: impl Into<String>, params: Vec<SqlValue>, changed: usize) {
        self.record(sql.into(), params, Answer::Changed(changed));
    }

    /// Records the rows the real database returned for `sql` with `params`.
    pub fn record_query(
        &self,
        sql: impl Into<String>,
        params: Vec<SqlValue>,
        rows: Vec<RecordBatch>,
    ) {
        self.record(sql.into(), params, Answer::Rows(rows));
    }

    /// Records that the real database reported `version` for `extension` once loaded.
    pub fn record_extension(&self, extension: Extension, version: impl Into<String>) {
        lock(&self.shared.extensions).insert(extension, version.into());
    }

    /// Returns every statement run with [`DatabaseConnection::execute`], with its
    /// parameters, in order, whether it succeeded or not.
    #[must_use]
    pub fn executed(&self) -> Vec<(String, Vec<SqlValue>)> {
        lock(&self.shared.executed).clone()
    }

    /// Returns the batches appended to `table`, in order.
    #[must_use]
    pub fn appended(&self, table: &str) -> Vec<RecordBatch> {
        lock(&self.shared.appended)
            .get(table)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns the DuckLake catalogs attached, in order.
    #[must_use]
    pub fn attached(&self) -> Vec<DuckLakeCatalog> {
        lock(&self.shared.attached).clone()
    }

    /// Returns the DuckLake catalogs attached with the `ENCRYPTED` option, in order.
    #[must_use]
    pub fn attached_encrypted(&self) -> Vec<DuckLakeCatalog> {
        lock(&self.shared.encrypted).clone()
    }

    /// Records that the DuckLake catalog whose metadata database is `metadata` exists and is
    /// not encrypted: attaching it with the `ENCRYPTED` option fails with
    /// [`PortError::UnencryptedCatalog`], as DuckLake refuses it (MG-REQ-0515.3).
    pub fn record_unencrypted(&self, metadata: impl Into<String>) {
        lock(&self.shared.unencrypted).push(metadata.into());
    }

    /// Records `answer` for `sql` with `params`, replacing an earlier one.
    fn record(&self, sql: String, params: Vec<SqlValue>, answer: Answer) {
        let mut answers = lock(&self.shared.answers);
        answers.retain(|(recorded, bound, _)| *recorded != sql || *bound != params);
        answers.push((sql, params, answer));
    }
}

impl Database for SimDatabase {
    fn connect(
        &self,
        _location: &DatabaseLocation,
    ) -> Result<Box<dyn DatabaseConnection>, PortError> {
        Ok(Box::new(SimConnection {
            shared: Arc::clone(&self.shared),
            loaded: Vec::new(),
        }))
    }
}

/// A connection of [`SimDatabase`].
struct SimConnection {
    shared: Arc<Shared>,
    loaded: Vec<LoadedExtension>,
}

impl DatabaseConnection for SimConnection {
    /// Fails with [`PortError::NotFound`] when the recorded result is rows, not a count.
    fn execute(&mut self, sql: &str, params: &[SqlValue]) -> Result<usize, PortError> {
        lock(&self.shared.executed).push((sql.to_owned(), params.to_vec()));
        match self.shared.answer(sql, params)? {
            Answer::Changed(changed) => Ok(changed),
            Answer::Rows(_) => Err(PortError::NotFound(format!("a recorded count for `{sql}`"))),
        }
    }

    /// Fails with [`PortError::NotFound`] when the recorded result is a count, not rows.
    fn query_arrow(
        &mut self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<RecordBatch>, PortError> {
        match self.shared.answer(sql, params)? {
            Answer::Rows(rows) => Ok(rows),
            Answer::Changed(_) => Err(PortError::NotFound(format!("recorded rows for `{sql}`"))),
        }
    }

    /// Replays the recorded batches one by one, each `each` call before the next batch.
    fn query_arrow_each(
        &mut self,
        sql: &str,
        params: &[SqlValue],
        each: &mut dyn FnMut(RecordBatch) -> Result<(), PortError>,
    ) -> Result<(), PortError> {
        for batch in self.query_arrow(sql, params)? {
            each(batch)?;
        }
        Ok(())
    }

    fn append_arrow(&mut self, table: &str, batch: &RecordBatch) -> Result<(), PortError> {
        self.shared.enter(table)?;
        lock(&self.shared.appended)
            .entry(table.to_owned())
            .or_default()
            .push(batch.clone());
        Ok(())
    }

    fn attach_ducklake(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.shared.enter(&catalog.alias)?;
        lock(&self.shared.attached).push(catalog.clone());
        Ok(())
    }

    /// Fails with [`PortError::UnencryptedCatalog`] for a catalog recorded with
    /// [`SimDatabase::record_unencrypted`].
    fn attach_ducklake_encrypted(&mut self, catalog: &DuckLakeCatalog) -> Result<(), PortError> {
        self.shared.enter(&catalog.alias)?;
        if lock(&self.shared.unencrypted).contains(&catalog.metadata) {
            return Err(PortError::UnencryptedCatalog {
                catalog: catalog.metadata.clone(),
            });
        }
        lock(&self.shared.encrypted).push(catalog.clone());
        Ok(())
    }

    /// Fails with [`PortError::NotFound`] when no version was recorded for `extension`.
    fn load_extension(&mut self, extension: Extension) -> Result<(), PortError> {
        self.shared.enter(extension.name())?;
        let version = lock(&self.shared.extensions)
            .get(&extension)
            .cloned()
            .ok_or_else(|| {
                PortError::NotFound(format!("a recorded version of `{}`", extension.name()))
            })?;
        self.loaded.retain(|loaded| loaded.extension != extension);
        self.loaded.push(LoadedExtension { extension, version });
        Ok(())
    }

    fn loaded_extensions(&self) -> Vec<LoadedExtension> {
        self.loaded.clone()
    }
}
