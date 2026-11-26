//! The harness an adapter provides, and the run of every check.

use std::sync::Arc;

use mergus_adapter::{AdapterError, StorageAdapter};

use crate::{Faults, Report, Status, concurrency, crash, operations, versions};

/// What an adapter gives the kit: fresh stores, and clients of a store.
pub trait Harness: Sync {
    /// One storage, shared by the clients opened on it.
    type Store: Sync;

    /// Returns a new, empty store.
    ///
    /// # Errors
    ///
    /// Returns an [`AdapterError`] when the store cannot be created.
    fn store(&self) -> Result<Self::Store, AdapterError>;

    /// Opens a new client of `store`. Before every storage call of the client, the harness
    /// MUST ask [`Faults::crashed`] and fail the call when it answers `true`.
    ///
    /// # Errors
    ///
    /// Returns an [`AdapterError`] when the client cannot be opened.
    fn open(
        &self,
        store: &Self::Store,
        faults: Arc<Faults>,
    ) -> Result<Box<dyn StorageAdapter>, AdapterError>;
}

/// One check: its name and what it runs.
type Check<H> = (&'static str, fn(&H) -> Result<Status, String>);

/// Returns every check of the kit, in the order they run.
fn checks<H: Harness>() -> Vec<Check<H>> {
    vec![
        ("capabilities", operations::capabilities::<H>),
        ("write append", operations::append::<H>),
        ("write replace", operations::replace::<H>),
        ("write replace period", operations::replace_period::<H>),
        ("write merge by key", operations::merge_by_key::<H>),
        ("write invisible before commit", operations::invisible::<H>),
        ("commit returns new versions", operations::new_versions::<H>),
        ("pin_recipe", operations::pin_recipe::<H>),
        ("read_at unknown version", operations::unknown_version::<H>),
        ("expire keeps live versions", versions::expire_live::<H>),
        (
            "expire keeps the rollback window",
            versions::expire_window::<H>,
        ),
        ("crash between write and commit", crash::sweep::<H>),
        ("client moves between threads", concurrency::moves::<H>),
        (
            "later client sees earlier commits",
            concurrency::sequential::<H>,
        ),
        ("concurrent clients", concurrency::concurrent::<H>),
    ]
}

/// Runs every check of the kit on the adapter of `harness` and reports each result. Each
/// check works on stores of its own (MG-REQ-0289).
#[must_use]
pub fn certify<H: Harness>(harness: &H) -> Report {
    let mut report = Report::default();
    for (name, check) in checks::<H>() {
        let status = check(harness).unwrap_or_else(Status::Failed);
        report.record(name, status);
    }
    report
}
