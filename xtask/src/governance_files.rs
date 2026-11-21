//! The `governance-files` check: the governance documents exist and are not empty, and the
//! code of conduct is the Contributor Covenant (MG-REQ-1749).

use std::fs;
use std::path::Path;

use crate::content::{self, Finding};
use crate::failure::Failure;

/// The check's name, which prefixes its findings.
const CHECK: &str = "governance-files";

/// The files of MG-REQ-1744, then `LICENSE-PLEDGE.md` (MG-REQ-1743), at the workspace root.
const FILES: [&str; 8] = [
    "LICENSE",
    "NOTICE",
    "CONTRIBUTING.md",
    "CODE_OF_CONDUCT.md",
    "SECURITY.md",
    "GOVERNANCE.md",
    "TRADEMARKS.md",
    "LICENSE-PLEDGE.md",
];

/// The code of conduct, and the text it must contain.
const CODE_OF_CONDUCT: (&str, &str) = ("CODE_OF_CONDUCT.md", "Contributor Covenant");

/// Checks the governance files at `root`.
///
/// # Errors
///
/// Fails with one line per finding, or if a file exists but cannot be read.
pub(crate) fn check(root: &Path) -> Result<(), Failure> {
    content::report(CHECK, &findings(root)?)
}

/// Returns the findings for the governance files at `root`, sorted by path.
///
/// # Errors
///
/// Fails if a file exists but cannot be read.
fn findings(root: &Path) -> Result<Vec<Finding>, Failure> {
    let mut findings = Vec::new();
    for file in FILES {
        let path = root.join(file);
        if !path.is_file() {
            findings.push(finding(file, "missing"));
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| Failure::io(&path, &error))?;
        if bytes.iter().all(u8::is_ascii_whitespace) {
            findings.push(finding(file, "empty"));
        } else if file == CODE_OF_CONDUCT.0
            && !String::from_utf8_lossy(&bytes).contains(CODE_OF_CONDUCT.1)
        {
            let message = format!("must contain \"{}\"", CODE_OF_CONDUCT.1);
            findings.push(finding(file, &message));
        }
    }
    findings.sort();
    Ok(findings)
}

/// A finding about a whole file.
fn finding(file: &str, problem: &str) -> Finding {
    Finding {
        path: file.to_owned(),
        line: 1,
        check: CHECK,
        message: format!("{problem} (MG-REQ-1749)"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::findings;
    use crate::workspace;

    fn fixture_findings(case: &str) -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("governance-files")
            .join(case);
        findings(&root)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn mg_req_1749_complete_governance_files_pass() {
        assert_eq!(fixture_findings("valid"), Vec::<String>::new());
    }

    #[test]
    fn mg_req_1749_this_repository_passes() {
        let root = workspace::root().unwrap();
        assert!(findings(&root).unwrap().is_empty());
    }

    #[test]
    fn mg_req_1749_missing_empty_or_wrong_files_fail() {
        assert_eq!(
            fixture_findings("broken"),
            [
                "CODE_OF_CONDUCT.md:1: governance-files: must contain \"Contributor Covenant\" \
                 (MG-REQ-1749)",
                "LICENSE:1: governance-files: missing (MG-REQ-1749)",
                "LICENSE-PLEDGE.md:1: governance-files: empty (MG-REQ-1749)",
                "NOTICE:1: governance-files: empty (MG-REQ-1749)",
                "SECURITY.md:1: governance-files: missing (MG-REQ-1749)",
            ]
        );
    }
}
