//! The checks of the six operations.

use mergus_adapter::{
    AdapterError, Requirement, VersionRef, Versioning, WriteMode, check_capabilities,
};

use crate::calls::{self, Client};
use crate::rows::{self, TABLE};
use crate::{Harness, Status};

/// The rows every check commits first.
pub(crate) const BASE: [i64; 2] = [1, 2];

/// A version no adapter returns.
const UNKNOWN: &str = "mergus-certify-unknown-version";

/// Opens a client on a new store and commits [`BASE`] with the base mode.
pub(crate) fn based<H: Harness>(harness: &H) -> Result<(Client, WriteMode, VersionRef), String> {
    let store = calls::store(harness)?;
    let mut client = calls::open(harness, &store)?;
    let mode = rows::base_mode(&client.capabilities())?;
    let base = calls::publish(&mut client, mode, &BASE)?;
    Ok((client, mode, base))
}

/// The declaration is the same on every call, declares a write mode, and passes the
/// capability check for everything it declares (MG-REQ-0287).
pub(crate) fn capabilities<H: Harness>(harness: &H) -> Result<Status, String> {
    let store = calls::store(harness)?;
    let client = calls::open(harness, &store)?;
    let declared = client.capabilities();
    if client.capabilities() != declared {
        return Err("capabilities changed between two calls".to_owned());
    }
    if declared.write_modes.is_empty() {
        return Err("no write mode is declared".to_owned());
    }
    let needs: Vec<Requirement> = declared
        .write_modes
        .iter()
        .map(|mode| Requirement::WriteMode(*mode))
        .chain(
            declared
                .exact_types
                .iter()
                .cloned()
                .map(Requirement::ExactType),
        )
        .collect();
    check_capabilities(&declared, &needs).map_err(|error| error.to_string())?;
    Ok(Status::Passed)
}

/// A declared write mode, applied to committed rows, gives the rows the mode says.
fn mode<H: Harness>(harness: &H, mode: WriteMode) -> Result<Status, String> {
    let (mut client, _, _) = based(harness)?;
    if !client.capabilities().write_modes.contains(&mode) {
        return Ok(Status::NotApplicable(format!(
            "write mode `{mode}` is not declared"
        )));
    }
    let rows = [3];
    let Some(expected) = rows::apply(mode, &BASE, &rows) else {
        return Ok(Status::NotApplicable(format!(
            "the contract does not carry the parameters of write mode `{mode}` yet"
        )));
    };
    let version = calls::publish(&mut client, mode, &rows)?;
    calls::expect_rows(&client, &version, &expected, "after the commit")?;
    Ok(Status::Passed)
}

/// The check of write mode append.
pub(crate) fn append<H: Harness>(harness: &H) -> Result<Status, String> {
    mode(harness, WriteMode::Append)
}

/// The check of write mode replace.
pub(crate) fn replace<H: Harness>(harness: &H) -> Result<Status, String> {
    mode(harness, WriteMode::Replace)
}

/// The check of write mode replace period.
pub(crate) fn replace_period<H: Harness>(harness: &H) -> Result<Status, String> {
    mode(harness, WriteMode::ReplacePeriod)
}

/// The check of write mode merge by key.
pub(crate) fn merge_by_key<H: Harness>(harness: &H) -> Result<Status, String> {
    mode(harness, WriteMode::MergeByKey)
}

/// Rows written and not committed are not in the published version (MG-REQ-0286).
pub(crate) fn invisible<H: Harness>(harness: &H) -> Result<Status, String> {
    let (mut client, mode, base) = based(harness)?;
    calls::write(&mut client, mode, &[3])?;
    calls::expect_rows(&client, &base, &BASE, "after a write, before its commit")?;
    Ok(Status::Passed)
}

/// The versions of a check with the rows of each, oldest first.
pub(crate) type History = Vec<(VersionRef, Vec<i64>)>;

/// Commits three versions after [`BASE`] and returns the client and every version with its
/// rows, oldest first. `None` when the adapter keeps no versions.
pub(crate) fn history<H: Harness>(harness: &H) -> Result<Option<(Client, History)>, String> {
    let (mut client, mode, base) = based(harness)?;
    if client.capabilities().versioning == Versioning::None {
        return Ok(None);
    }
    let mut versions = vec![(base, BASE.to_vec())];
    for id in 3..6 {
        let before = versions
            .last()
            .map(|(_, ids)| ids.clone())
            .unwrap_or_default();
        let expected = rows::apply(mode, &before, &[id]).unwrap_or_default();
        versions.push((calls::publish(&mut client, mode, &[id])?, expected));
    }
    Ok(Some((client, versions)))
}

/// Every commit returns a version no earlier commit returned, and each stays readable.
pub(crate) fn new_versions<H: Harness>(harness: &H) -> Result<Status, String> {
    let Some((client, versions)) = history(harness)? else {
        return Ok(Status::NotApplicable(
            "the adapter keeps no versions".to_owned(),
        ));
    };
    for (index, (version, expected)) in versions.iter().enumerate() {
        if versions[..index]
            .iter()
            .any(|(earlier, _)| earlier == version)
        {
            return Err(format!(
                "commit returned version `{}` twice",
                version.as_str()
            ));
        }
        calls::expect_rows(&client, version, expected, "after later commits")?;
    }
    Ok(Status::Passed)
}

/// `pin_recipe` describes every committed version and refuses a version it does not know.
pub(crate) fn pin_recipe<H: Harness>(harness: &H) -> Result<Status, String> {
    let (client, _, base) = based(harness)?;
    calls::pin(&client, &base).map_err(|error| {
        format!(
            "pin_recipe of committed version `{}` failed: {error}",
            base.as_str()
        )
    })?;
    if let Ok(recipe) = calls::pin(&client, &VersionRef::new(UNKNOWN)) {
        return Err(format!(
            "pin_recipe of unknown version `{UNKNOWN}` returned `{}`",
            recipe.as_str()
        ));
    }
    Ok(Status::Passed)
}

/// Reading at a version the storage does not know fails with
/// [`AdapterError::VersionUnavailable`] naming the table and the version (MG-REQ-0252).
pub(crate) fn unknown_version<H: Harness>(harness: &H) -> Result<Status, String> {
    let (client, _, _) = based(harness)?;
    expect_unavailable(&client, &VersionRef::new(UNKNOWN))?;
    Ok(Status::Passed)
}

/// Fails unless reading the kit's table at `version` fails with `VersionUnavailable`.
pub(crate) fn expect_unavailable(client: &Client, version: &VersionRef) -> Result<(), String> {
    match client.read_at(TABLE, version) {
        Err(AdapterError::VersionUnavailable { table, version: at })
            if table == TABLE && at == *version =>
        {
            Ok(())
        }
        Err(error) => Err(format!(
            "read_at version `{}` failed without naming it unavailable: {error}",
            version.as_str()
        )),
        Ok(_) => Err(format!(
            "read_at version `{}` succeeded; it must be unavailable",
            version.as_str()
        )),
    }
}
