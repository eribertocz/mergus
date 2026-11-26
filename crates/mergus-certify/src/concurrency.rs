//! The checks of concurrency: clients across threads, one after the other, and at once.

use std::thread;

use mergus_adapter::WriteMode;

use crate::calls::{self, Client};
use crate::operations::{BASE, based};
use crate::{Harness, Status, rows};

/// The commits each concurrent client makes.
const ROUNDS: i64 = 5;

/// Runs `work` on another thread and returns its result.
fn on_thread<T: Send>(work: impl FnOnce() -> Result<T, String> + Send) -> Result<T, String> {
    thread::scope(|scope| {
        scope
            .spawn(work)
            .join()
            .unwrap_or_else(|_| Err("the adapter panicked on another thread".to_owned()))
    })
}

/// A client written on one thread commits on another (the contract requires `Send`).
pub(crate) fn moves<H: Harness>(harness: &H) -> Result<Status, String> {
    let (client, mode, _) = based(harness)?;
    let mut client: Client = on_thread(move || {
        let mut client = client;
        calls::write(&mut client, mode, &[3])?;
        Ok(client)
    })?;
    let version = calls::commit(&mut client)?;
    let expected = rows::apply(mode, &BASE, &[3]).unwrap_or_default();
    calls::expect_rows(
        &client,
        &version,
        &expected,
        "after a commit on another thread",
    )?;
    Ok(Status::Passed)
}

/// A client opened after another one closed sees its commits and builds on them.
pub(crate) fn sequential<H: Harness>(harness: &H) -> Result<Status, String> {
    let store = calls::store(harness)?;
    let mut first = calls::open(harness, &store)?;
    let mode = rows::base_mode(&first.capabilities())?;
    let base = calls::publish(&mut first, mode, &BASE)?;
    drop(first);
    let mut second = calls::open(harness, &store)?;
    calls::expect_rows(&second, &base, &BASE, "from a later client")?;
    let version = calls::publish(&mut second, mode, &[3])?;
    let expected = rows::apply(mode, &BASE, &[3]).unwrap_or_default();
    calls::expect_rows(
        &second,
        &version,
        &expected,
        "after a later client's commit",
    )?;
    Ok(Status::Passed)
}

/// When concurrent clients are declared, two clients append and commit at once and no row
/// is lost.
pub(crate) fn concurrent<H: Harness>(harness: &H) -> Result<Status, String> {
    let store = calls::store(harness)?;
    let mut last = calls::open(harness, &store)?;
    let capabilities = last.capabilities();
    if !capabilities.concurrent_clients {
        return Ok(Status::NotApplicable(
            "concurrent clients are not declared".to_owned(),
        ));
    }
    if !capabilities.write_modes.contains(&WriteMode::Append) {
        return Ok(Status::NotApplicable(
            "write mode `append` is not declared".to_owned(),
        ));
    }
    let clients = [calls::open(harness, &store)?, calls::open(harness, &store)?];
    thread::scope(|scope| {
        let handles: Vec<_> = clients
            .into_iter()
            .zip([100, 200])
            .map(|(client, first)| scope.spawn(move || append_rounds(client, first)))
            .collect();
        handles.into_iter().try_for_each(|handle| {
            handle
                .join()
                .unwrap_or_else(|_| Err("the adapter panicked on another thread".to_owned()))
        })
    })?;
    let version = calls::publish(&mut last, WriteMode::Append, &[0])?;
    let mut expected: Vec<i64> = (0..ROUNDS)
        .flat_map(|round| [100 + round, 200 + round])
        .collect();
    expected.push(0);
    expected.sort_unstable();
    calls::expect_rows(&last, &version, &expected, "after concurrent commits")?;
    Ok(Status::Passed)
}

/// Appends and commits `first`, `first + 1`… for [`ROUNDS`] rounds.
fn append_rounds(mut client: Client, first: i64) -> Result<(), String> {
    for round in 0..ROUNDS {
        calls::publish(&mut client, WriteMode::Append, &[first + round])?;
    }
    Ok(())
}
