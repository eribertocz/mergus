//! The environment of the Python process (MG-REQ-0006, MG-REQ-1003).

use mergus_ports::Environment;

/// The variables of the current environment that reach the Python process, when set and
/// valid Unicode. No other variable does.
const PASSED: [&str; 6] = ["PATH", "LANG", "LC_ALL", "TZ", "SYSTEMROOT", "VIRTUAL_ENV"];

/// The variables Mergus sets itself, whatever the current environment holds.
const SET: [(&str, &str); 2] = [("PYTHONHASHSEED", "0"), ("PYTHONNOUSERSITE", "1")];

/// The variables that point to the work directory of the run.
const TEMPORARY: [&str; 3] = ["TMP", "TEMP", "TMPDIR"];

/// Returns the environment the Python process starts with, sorted by name. It is an
/// allowlist: `PATH`, `LANG`, `LC_ALL`, `TZ`, `SYSTEMROOT` and `VIRTUAL_ENV`, read from
/// `environment` when set, then `PYTHONHASHSEED=0`, `PYTHONNOUSERSITE=1`, and `TMP`, `TEMP`
/// and `TMPDIR` set to `work_dir`, the run's work directory. No `MERGUS_SECRET_*` variable
/// and no cloud SDK credential can reach Python (MG-REQ-1003).
///
/// The process must start with only these variables (`ProcessCommand::clear_env`), so that
/// nothing is inherited around them (MG-REQ-0006).
#[must_use]
pub fn python_environment(environment: &dyn Environment, work_dir: &str) -> Vec<(String, String)> {
    let passed = PASSED
        .iter()
        .filter_map(|name| Some(((*name).to_owned(), environment.var(name)?)));
    let set = SET
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()));
    let temporary = TEMPORARY
        .iter()
        .map(|name| ((*name).to_owned(), work_dir.to_owned()));
    let mut vars: Vec<(String, String)> = passed.chain(set).chain(temporary).collect();
    vars.sort();
    vars
}
