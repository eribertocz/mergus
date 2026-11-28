//! Idempotency keys (MG-REQ-1313): a command that changes state, given
//! `--idempotency-key <key>`, keeps its result for 24 hours in
//! `.mergus/idempotency/<sha256(key)>.json`, and a second call with the same key and the same
//! arguments prints that result again and changes nothing.

use std::io::Write;
use std::path::{Path, PathBuf};

use mergus_contracts::canonical_json;
use mergus_contracts::hash::sha256_hex;
use serde_json::{Value, json};

use crate::args::Invocation;
use crate::commands::IDEMPOTENCY_KEY;
use crate::error::CliError;
use crate::project;
use crate::run::Ports;

/// The folder of the records, relative to the project root, or to the current directory
/// outside a project.
const FOLDER: &str = ".mergus/idempotency";

/// How long a key lives, in seconds.
const LIFETIME_SECONDS: i64 = 24 * 3600;

/// One call's result, as its record keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: u8,
}

/// What a key found before the command runs.
pub(crate) enum Found {
    /// The record of the same arguments: its result is the answer.
    Replay(Outcome),
    /// A live record of other arguments: the command must fail (MG-1314).
    Conflict(CliError),
    /// No live record: the command runs, and its result is kept at this path.
    Fresh(PathBuf),
}

/// Looks up the key of `invocation`, which the caller checked is given.
///
/// # Errors
///
/// Fails with [`CliError::Unreadable`] when the current directory or the record cannot be
/// read.
pub(crate) fn find(ports: &Ports<'_>, invocation: &Invocation) -> Result<Found, CliError> {
    let key = invocation.value(IDEMPOTENCY_KEY).unwrap_or_default();
    let path = folder(ports)?.join(format!("{}.json", sha256_hex(key.as_bytes())));
    let unreadable = |message: String| CliError::Unreadable(message);
    if !ports
        .files
        .exists(&path)
        .map_err(|error| unreadable(error.to_string()))?
    {
        return Ok(Found::Fresh(path));
    }
    let bytes = ports
        .files
        .read(&path)
        .map_err(|error| unreadable(error.to_string()))?;
    let Ok(record) = serde_json::from_slice::<Value>(&bytes) else {
        // A record that is not ours is replaced, as an expired one is.
        return Ok(Found::Fresh(path));
    };
    let created = record["created_unix_seconds"].as_i64().unwrap_or(i64::MIN);
    if now_seconds(ports).saturating_sub(created) > LIFETIME_SECONDS {
        return Ok(Found::Fresh(path));
    }
    if record["arguments"].as_str() != Some(arguments_hash(invocation).as_str()) {
        return Ok(Found::Conflict(CliError::IdempotencyConflict {
            key: key.to_owned(),
        }));
    }
    let text = |name: &str| {
        record[name]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .to_vec()
    };
    let exit_code = record["exit_code"]
        .as_u64()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or_default();
    Ok(Found::Replay(Outcome {
        stdout: text("stdout"),
        stderr: text("stderr"),
        exit_code,
    }))
}

/// Keeps `outcome` at `path`, the record of the key of `invocation`.
///
/// # Errors
///
/// Fails with [`CliError::Unwritable`] when the record cannot be written.
pub(crate) fn keep(
    ports: &Ports<'_>,
    invocation: &Invocation,
    path: &Path,
    outcome: &Outcome,
) -> Result<(), CliError> {
    let record = json!({
        "arguments": arguments_hash(invocation),
        "created_unix_seconds": now_seconds(ports),
        "exit_code": outcome.exit_code,
        "stdout": String::from_utf8_lossy(&outcome.stdout),
        "stderr": String::from_utf8_lossy(&outcome.stderr),
    });
    let unwritable = |error: mergus_ports::PortError| CliError::Unwritable {
        path: format!("{FOLDER}/{}", file_name(path)),
        message: error.to_string(),
    };
    if let Some(parent) = path.parent() {
        ports.files.create_dir_all(parent).map_err(unwritable)?;
    }
    ports
        .files
        .write(path, canonical_json(&record).as_bytes())
        .map_err(unwritable)
}

/// Writes `outcome` to the outputs of the call.
///
/// # Errors
///
/// Fails with [`CliError::Internal`] when an output cannot be written.
pub(crate) fn replay(
    outcome: &Outcome,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), CliError> {
    let failed = |error: std::io::Error| {
        CliError::Internal(format!("the output cannot be written: {error}"))
    };
    stdout.write_all(&outcome.stdout).map_err(failed)?;
    stdout.flush().map_err(failed)?;
    stderr.write_all(&outcome.stderr).map_err(failed)?;
    stderr.flush().map_err(failed)
}

/// The folder of the records: under the project root, or under the current directory when
/// there is no project yet, as for `init`.
fn folder(ports: &Ports<'_>) -> Result<PathBuf, CliError> {
    let root = match project::find(ports.environment, ports.files) {
        Ok(root) => root,
        Err(CliError::ProjectNotFound { .. }) => ports
            .environment
            .current_dir()
            .map_err(|error| CliError::Unreadable(error.to_string()))?,
        Err(error) => return Err(error),
    };
    Ok(FOLDER.split('/').fold(root, |path, part| path.join(part)))
}

/// The hash of the canonical arguments: the command and every argument given but the key,
/// sorted, so that their order and the `--name=value` spelling do not count.
fn arguments_hash(invocation: &Invocation) -> String {
    let mut given: Vec<(&str, &str)> = invocation
        .given()
        .iter()
        .filter(|(name, _)| *name != IDEMPOTENCY_KEY)
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    given.sort_unstable();
    let canonical = json!({
        "command": invocation.command.name(),
        "arguments": given,
    });
    sha256_hex(canonical_json(&canonical).as_bytes())
}

fn now_seconds(ports: &Ports<'_>) -> i64 {
    let seconds = ports.clock.now().unix_nanos() / 1_000_000_000;
    i64::try_from(seconds).unwrap_or(i64::MAX)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
