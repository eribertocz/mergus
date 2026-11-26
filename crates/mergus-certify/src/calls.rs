//! The adapter calls of the checks, with the context of a failure.

use std::sync::Arc;

use mergus_adapter::{PinRecipe, StorageAdapter, VersionRef, WriteMode};

use crate::rows::{self, TABLE};
use crate::{Faults, Harness};

/// A client of the adapter under certification.
pub(crate) type Client = Box<dyn StorageAdapter>;

/// Returns a new, empty store.
pub(crate) fn store<H: Harness>(harness: &H) -> Result<H::Store, String> {
    harness
        .store()
        .map_err(|error| format!("the harness cannot create a store: {error}"))
}

/// Opens a client of `store` that never crashes.
pub(crate) fn open<H: Harness>(harness: &H, store: &H::Store) -> Result<Client, String> {
    open_with(harness, store, Faults::new())
}

/// Opens a client of `store` that crashes as `faults` says.
pub(crate) fn open_with<H: Harness>(
    harness: &H,
    store: &H::Store,
    faults: Arc<Faults>,
) -> Result<Client, String> {
    harness
        .open(store, faults)
        .map_err(|error| format!("open failed: {error}"))
}

/// Writes `ids` to the kit's table with `mode`.
pub(crate) fn write(client: &mut Client, mode: WriteMode, ids: &[i64]) -> Result<(), String> {
    client
        .write(TABLE, mode, rows::stream(ids)?)
        .map_err(|error| format!("write with mode `{mode}` failed: {error}"))
}

/// Commits and returns the new version.
pub(crate) fn commit(client: &mut Client) -> Result<VersionRef, String> {
    client
        .commit()
        .map_err(|error| format!("commit failed: {error}"))
}

/// Writes `ids` with `mode`, commits and returns the new version.
pub(crate) fn publish(
    client: &mut Client,
    mode: WriteMode,
    ids: &[i64],
) -> Result<VersionRef, String> {
    write(client, mode, ids)?;
    commit(client)
}

/// Returns the ids of the kit's table at `version`, sorted.
pub(crate) fn read(client: &Client, version: &VersionRef) -> Result<Vec<i64>, String> {
    let stream = client
        .read_at(TABLE, version)
        .map_err(|error| format!("read_at version `{}` failed: {error}", version.as_str()))?;
    rows::ids(stream)
}

/// Fails unless the kit's table holds exactly `expected` at `version`.
pub(crate) fn expect_rows(
    client: &Client,
    version: &VersionRef,
    expected: &[i64],
    when: &str,
) -> Result<(), String> {
    let found = read(client, version)?;
    if found == expected {
        Ok(())
    } else {
        Err(format!(
            "{when}: version `{}` holds {found:?}, expected {expected:?}",
            version.as_str()
        ))
    }
}

/// Returns the recipe to read the kit's table pinned at `version`. The only place the kit
/// calls `pin_recipe`.
pub(crate) fn pin(
    client: &Client,
    version: &VersionRef,
) -> Result<PinRecipe, mergus_adapter::AdapterError> {
    client.pin_recipe(TABLE, version)
}
