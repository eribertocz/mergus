//! The crash points between write and commit.

use mergus_adapter::{VersionRef, Versioning, WriteMode};

use crate::calls;
use crate::operations::BASE;
use crate::{Faults, Harness, Status, rows};

/// The rows written when the client crashes: more than one, so a partial commit shows.
const NEW: [i64; 2] = [3, 4];

/// The rows a client writes after the crash, to see what the crash published.
const MARK: [i64; 1] = [9];

/// The storage calls a write and its commit may take before the kit gives up.
const MAX_CALLS: u64 = 10_000;

/// Crashes the client at its first, second, third… storage call after the base commit,
/// through the write, the time between write and commit, and the commit, until a write and
/// its commit complete without a crash. After each crash a new client finds the published
/// version intact and the crashed write published whole or not at all.
pub(crate) fn sweep<H: Harness>(harness: &H) -> Result<Status, String> {
    for point in 0..MAX_CALLS {
        let completed = crash_at(harness, point).map_err(|error| {
            format!("crash at storage call {point} after the write starts: {error}")
        })?;
        if completed {
            return Ok(Status::Passed);
        }
    }
    Err(format!(
        "a write and its commit did not complete within {MAX_CALLS} storage calls"
    ))
}

/// Crashes the client at storage call `point` from the write on, then recovers. Returns
/// whether the write and its commit completed before the crash point.
fn crash_at<H: Harness>(harness: &H, point: u64) -> Result<bool, String> {
    let store = calls::store(harness)?;
    let faults = Faults::new();
    let mut client = calls::open_with(harness, &store, std::sync::Arc::clone(&faults))?;
    let capabilities = client.capabilities();
    let mode = rows::base_mode(&capabilities)?;
    let base = calls::publish(&mut client, mode, &BASE)?;
    faults.crash_after(point);
    let written = calls::write(&mut client, mode, &NEW).and_then(|()| calls::commit(&mut client));
    let crashed = faults.has_crashed();
    drop(client);
    if !crashed {
        let version = written?;
        let reader = calls::open(harness, &store)?;
        let expected = rows::apply(mode, &BASE, &NEW).unwrap_or_default();
        calls::expect_rows(&reader, &version, &expected, "without a crash")?;
        return Ok(true);
    }
    let versioned = capabilities.versioning != Versioning::None;
    recover(harness, &store, mode, versioned.then_some(&base))?;
    Ok(false)
}

/// Opens a new client after a crash: `base`, when versions are kept, still holds its rows,
/// and the next commit holds the crashed write whole or not at all.
fn recover<H: Harness>(
    harness: &H,
    store: &H::Store,
    mode: WriteMode,
    base: Option<&VersionRef>,
) -> Result<(), String> {
    let mut client = calls::open(harness, store)?;
    if let Some(base) = base {
        calls::expect_rows(&client, base, &BASE, "after the crash")?;
    }
    let version = calls::publish(&mut client, mode, &MARK)?;
    let found = calls::read(&client, &version)?;
    let lost = rows::apply(mode, &BASE, &MARK).unwrap_or_default();
    let published = rows::apply(mode, &BASE, &NEW)
        .and_then(|after| rows::apply(mode, &after, &MARK))
        .unwrap_or_default();
    if found == lost || found == published {
        Ok(())
    } else {
        Err(format!(
            "after the crash, version `{}` holds {found:?}: the crashed write is partly \
             published (expected {lost:?} or {published:?})",
            version.as_str()
        ))
    }
}
