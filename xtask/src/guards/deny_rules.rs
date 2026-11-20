//! The `deny-rules` guard: `.claude/settings.json` at the head parses and keeps every entry
//! of `permissions.deny` at the base (MG-REQ-1737).
//!
//! It needs no network: it reads both versions of the file with `git`.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::failure::Failure;
use crate::json::Json;
use crate::workspace;

/// The settings file, relative to the repository root.
pub(crate) const SETTINGS: &str = ".claude/settings.json";

/// Returns the findings of the `deny-rules` guard on the base text of the settings and the
/// head text, `None` when the head has no such file.
///
/// # Errors
///
/// Fails if the base text does not parse.
pub(crate) fn judge(base: &str, head: Option<&str>) -> Result<Vec<String>, Failure> {
    let base = Json::parse(base)
        .map_err(|failure| Failure::new(format!("{SETTINGS} at the base: {failure:?}")))?;
    let head = match head.map(Json::parse) {
        Some(Ok(head)) => head,
        Some(Err(failure)) => return Ok(vec![unparsable(&format!("{failure:?}"))]),
        None => return Ok(vec![unparsable("the file is missing")]),
    };
    let kept = deny_list(&head);
    Ok(deny_list(&base)
        .into_iter()
        .filter(|entry| !kept.contains(entry))
        .map(|entry| {
            format!(
                "MG-REQ-1737: permissions.deny at the head lacks `{entry}`, present at the base"
            )
        })
        .collect())
}

/// The finding of a head file that does not parse.
fn unparsable(reason: &str) -> String {
    format!("MG-REQ-1737: {SETTINGS} at the head does not parse: {reason}")
}

/// The string entries of `permissions.deny`, in order.
fn deny_list(settings: &Json) -> Vec<&str> {
    settings
        .get("permissions")
        .and_then(|permissions| permissions.get("deny"))
        .map(Json::items)
        .unwrap_or_default()
        .iter()
        .filter_map(Json::as_str)
        .collect()
}

/// Reads the settings at the commit `revision`, `None` when the commit has no such file.
///
/// # Errors
///
/// Fails if `git` cannot run or the revision does not exist.
pub(crate) fn at_revision(root: &Path, revision: &str) -> Result<Option<String>, Failure> {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .stderr(Stdio::null())
            .output()
            .map_err(|error| Failure::new(format!("cannot run git: {error}")))
    };
    let commit = format!("{revision}^{{commit}}");
    if !git(&["rev-parse", "--verify", "--quiet", &commit])?
        .status
        .success()
    {
        return Err(Failure::new(format!("git has no commit `{revision}`")));
    }
    let output = git(&["show", &format!("{revision}:{SETTINGS}")])?;
    if !output.status.success() {
        return Ok(None);
    }
    String::from_utf8(output.stdout)
        .map(Some)
        .map_err(|_| Failure::new(format!("{SETTINGS} at `{revision}` is not UTF-8")))
}

/// Reads the settings in the working tree, `None` when the file does not exist.
///
/// # Errors
///
/// Fails if the file exists but cannot be read.
pub(crate) fn in_working_tree(root: &Path) -> Result<Option<String>, Failure> {
    let path = root.join(SETTINGS);
    if path.exists() {
        workspace::read(&path).map(Some)
    } else {
        Ok(None)
    }
}
