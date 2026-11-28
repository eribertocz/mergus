//! Idempotency keys (MG-REQ-1313): a command that changes state, given
//! `--idempotency-key <key>`, keeps its result for 24 hours in
//! `.mergus/idempotency/<sha256(key)>.json`, and a second call with the same key and the same
//! arguments prints that result again and changes nothing. A call claims its key before it
//! runs, and a retryable result releases the claim instead of being kept.

use std::io::Write;
use std::path::{Path, PathBuf};

use mergus_contracts::canonical_json;
use mergus_contracts::hash::sha256_hex;
use serde_json::{Value, json};

use crate::args::Invocation;
use crate::commands::IDEMPOTENCY_KEY;
use crate::error::CliError;
use crate::exit;
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
    /// A live record of other arguments (MG-1314), or the claim of a call still running
    /// (MG-1315): the command must fail and do nothing.
    Refused(CliError),
    /// No live record: this call claimed the key, and keeps its result at this path.
    Claimed(PathBuf),
}

/// Looks up the key of `invocation`, which the caller checked is given, and claims it when no
/// live record holds it (MG-REQ-1313.2).
///
/// # Errors
///
/// Fails with [`CliError::Unreadable`] when the current directory or the record cannot be
/// read, and with [`CliError::Unwritable`] when the claim cannot be written.
pub(crate) fn find(ports: &Ports<'_>, invocation: &Invocation) -> Result<Found, CliError> {
    let key = invocation.value(IDEMPOTENCY_KEY).unwrap_or_default();
    let path = folder(ports)?.join(format!("{}.json", sha256_hex(key.as_bytes())));
    if claim(ports, invocation, &path)? {
        return Ok(Found::Claimed(path));
    }
    let claimed = || {
        Found::Refused(CliError::IdempotencyClaimed {
            key: key.to_owned(),
        })
    };
    let Some(record) = live(ports, &path)? else {
        // An expired record, one that is not ours, or a claim just released, is replaced. A
        // call that claims it first wins.
        match ports.files.remove_file(&path) {
            Ok(()) | Err(mergus_ports::PortError::NotFound(_)) => {}
            Err(error) => return Err(unwritable(&path, &error)),
        }
        return Ok(if claim(ports, invocation, &path)? {
            Found::Claimed(path)
        } else {
            claimed()
        });
    };
    if record["state"].as_str() == Some(RUNNING) {
        return Ok(claimed());
    }
    if record["arguments"].as_str() != Some(arguments_hash(invocation).as_str()) {
        return Ok(Found::Refused(CliError::IdempotencyConflict {
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

/// Whether the result of a call that ended with `exit_code` is kept: a retryable one, 3, 4 or
/// 7, is not, so that a retry runs the command again (MG-REQ-1313.3).
pub(crate) fn is_kept(exit_code: u8) -> bool {
    !matches!(exit_code, 3 | 4 | exit::CANCELLED)
}

/// Releases the claim at `path`, so that the key is free again (MG-REQ-1313.3).
///
/// # Errors
///
/// Fails with [`CliError::Unwritable`] when the claim cannot be removed.
pub(crate) fn release(ports: &Ports<'_>, path: &Path) -> Result<(), CliError> {
    ports
        .files
        .remove_file(path)
        .map_err(|error| unwritable(path, &error))
}

/// The state of a claim, which a call writes before it runs and replaces with its result.
const RUNNING: &str = "running";

/// Creates the claim of `invocation` at `path` if nothing is there, and returns whether it
/// did.
fn claim(ports: &Ports<'_>, invocation: &Invocation, path: &Path) -> Result<bool, CliError> {
    let record = json!({
        "arguments": arguments_hash(invocation),
        "created_unix_seconds": now_seconds(ports),
        "state": RUNNING,
    });
    if let Some(parent) = path.parent() {
        ports
            .files
            .create_dir_all(parent)
            .map_err(|error| unwritable(path, &error))?;
    }
    ports
        .files
        .create_new(path, canonical_json(&record).as_bytes())
        .map_err(|error| unwritable(path, &error))
}

/// The record at `path` when it is ours and younger than 24 hours, a claim or a result.
fn live(ports: &Ports<'_>, path: &Path) -> Result<Option<Value>, CliError> {
    let bytes = match ports.files.read(path) {
        Ok(bytes) => bytes,
        Err(mergus_ports::PortError::NotFound(_)) => return Ok(None),
        Err(error) => return Err(CliError::Unreadable(error.to_string())),
    };
    let Ok(record) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(None);
    };
    let created = record["created_unix_seconds"].as_i64().unwrap_or(i64::MIN);
    let expired = now_seconds(ports).saturating_sub(created) > LIFETIME_SECONDS;
    Ok((!expired).then_some(record))
}

/// Keeps `outcome` at `path`, the record of the key of `invocation`, in place of its claim.
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
    let unwritable = |error: mergus_ports::PortError| unwritable(path, &error);
    // Written beside it, then moved into place, so that a record is whole or absent: a
    // truncated one would read as no record, and the key would run the command again.
    let temporary = path.with_extension("json.tmp");
    ports
        .files
        .write(&temporary, canonical_json(&record).as_bytes())
        .map_err(unwritable)?;
    ports.files.rename(&temporary, path).map_err(unwritable)
}

/// A writer that writes to `out` at once and keeps a copy of everything it wrote, so that a
/// keyed command still streams its output (MG-REQ-0710.2) and its record holds all of it.
pub(crate) struct Tee<'a> {
    out: &'a mut dyn Write,
    /// Everything written so far.
    pub(crate) copy: Vec<u8>,
}

impl<'a> Tee<'a> {
    /// A writer to `out`, with an empty copy.
    pub(crate) fn new(out: &'a mut dyn Write) -> Self {
        Self {
            out,
            copy: Vec::new(),
        }
    }
}

impl Write for Tee<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let written = self.out.write(bytes)?;
        self.copy
            .extend_from_slice(bytes.get(..written).unwrap_or_default());
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
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

/// The failure to write the record at `path`.
fn unwritable(path: &Path, error: &mergus_ports::PortError) -> CliError {
    CliError::Unwritable {
        path: format!("{FOLDER}/{}", file_name(path)),
        message: error.to_string(),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
