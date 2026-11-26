//! The checks of `expire` (MG-REQ-0288).

use std::time::Duration;

use mergus_adapter::Retention;

use crate::calls::{self, Client};
use crate::operations::{expect_unavailable, history};
use crate::{Harness, Status};

/// A rollback window every version of a check is inside.
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// Runs `expire` with `retention`.
fn expire(client: &mut Client, retention: &Retention) -> Result<(), String> {
    client
        .expire(retention)
        .map_err(|error| format!("expire failed: {error}"))
}

/// Outside the rollback window, `expire` keeps the live versions and deletes the others.
pub(crate) fn expire_live<H: Harness>(harness: &H) -> Result<Status, String> {
    let Some((mut client, versions)) = history(harness)? else {
        return Ok(Status::NotApplicable(
            "the adapter keeps no versions".to_owned(),
        ));
    };
    // The oldest and the newest are live; the two between are not.
    let live = [0, versions.len() - 1];
    let retention = Retention {
        live: live
            .iter()
            .map(|index| versions[*index].0.clone())
            .collect(),
        rollback_window: Duration::ZERO,
    };
    expire(&mut client, &retention)?;
    for (index, (version, expected)) in versions.iter().enumerate() {
        if live.contains(&index) {
            calls::expect_rows(&client, version, expected, "after expire, live")?;
        } else {
            expect_unavailable(&client, version)?;
        }
    }
    Ok(Status::Passed)
}

/// `expire` keeps every version inside the rollback window, live or not.
pub(crate) fn expire_window<H: Harness>(harness: &H) -> Result<Status, String> {
    let Some((mut client, versions)) = history(harness)? else {
        return Ok(Status::NotApplicable(
            "the adapter keeps no versions".to_owned(),
        ));
    };
    let retention = Retention {
        live: Vec::new(),
        rollback_window: DAY,
    };
    expire(&mut client, &retention)?;
    for (version, expected) in &versions {
        calls::expect_rows(
            &client,
            version,
            expected,
            "after expire, inside the window",
        )?;
    }
    Ok(Status::Passed)
}
