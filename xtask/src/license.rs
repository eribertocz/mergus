//! The `license-headers` check: every workspace member declares its license in its
//! `Cargo.toml` (MG-REQ-1741, MG-REQ-1747). Source files carry no license header.

use std::path::Path;

use crate::content::{self, Finding};
use crate::failure::Failure;
use crate::workspace;

/// The check's name, which prefixes its findings.
const CHECK: &str = "license-headers";

/// The license of a member that is not published.
const LICENSE: &str = "Apache-2.0";

/// The license of a member that may be published for reuse.
const PUBLISHED_LICENSE: &str = "MIT OR Apache-2.0";

/// Checks the license of every member of the workspace at `root`.
///
/// # Errors
///
/// Fails with one line per finding, or if a manifest cannot be read.
pub(crate) fn check(root: &Path) -> Result<(), Failure> {
    content::report(CHECK, &findings(root)?)
}

/// Returns the findings for every member of the workspace at `root`, sorted by path.
///
/// # Errors
///
/// Fails if a manifest cannot be read.
fn findings(root: &Path) -> Result<Vec<Finding>, Failure> {
    let workspace_manifest = workspace::read(&root.join("Cargo.toml"))?;
    let inherited = Fields::of(&workspace_manifest, "workspace.package");
    let mut findings = Vec::new();
    for member in workspace::members(root)? {
        let path = member.dir.join("Cargo.toml");
        let fields = Fields::of(&workspace::read(&path)?, "package");
        let license = fields.license.resolve(&inherited.license);
        let published = fields.publish.resolve(&inherited.publish) != Value::Bool(false);
        let (expected, why) = if published {
            (PUBLISHED_LICENSE, "may be published, so it ")
        } else {
            (LICENSE, "")
        };
        if license != Value::Text(expected.to_owned()) {
            findings.push(Finding {
                path: workspace::show(root, &path),
                line: 1,
                check: CHECK,
                message: format!(
                    "{why}must declare `license = \"{expected}\"`, directly or through \
                     `license.workspace = true` (MG-REQ-1747)"
                ),
            });
        }
    }
    findings.sort();
    Ok(findings)
}

/// The value of a key of a manifest, as far as this check reads it.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    /// The key is absent, or its value has a form this check does not read.
    Absent,
    /// `true` or `false`.
    Bool(bool),
    /// A string.
    Text(String),
    /// `key.workspace = true` or `key = { workspace = true }`: the workspace's value.
    Inherited,
}

impl Value {
    /// The value itself, or `inherited` when it is inherited from the workspace.
    fn resolve(&self, inherited: &Self) -> Self {
        match self {
            Self::Inherited => inherited.clone(),
            other => other.clone(),
        }
    }
}

/// The keys `license` and `publish` of one table of a manifest.
struct Fields {
    license: Value,
    publish: Value,
}

impl Fields {
    /// Reads `license` and `publish` in the table `[<table>]` of `manifest`.
    fn of(manifest: &str, table: &str) -> Self {
        let header = format!("[{table}]");
        let mut fields = Self {
            license: Value::Absent,
            publish: Value::Absent,
        };
        let mut inside = false;
        for line in manifest.lines().map(str::trim) {
            if line.starts_with('[') {
                inside = line.split('#').next().unwrap_or_default().trim() == header;
                continue;
            }
            let Some((key, value)) = line.split_once('=').filter(|_| inside) else {
                continue;
            };
            let compact: String = key.split_whitespace().collect();
            for (name, field) in [
                ("license", &mut fields.license),
                ("publish", &mut fields.publish),
            ] {
                if let Some(value) = read_value(name, &compact, value) {
                    *field = value;
                }
            }
        }
        fields
    }
}

/// The value of the key `name` when the line's key is `compact` and its value `value`, or
/// `None` when the line sets another key.
fn read_value(name: &str, compact: &str, value: &str) -> Option<Value> {
    let value = value.trim();
    if compact == format!("{name}.workspace") {
        return Some(if starts_with_word(value, "true") {
            Value::Inherited
        } else {
            Value::Absent
        });
    }
    if compact != name {
        return None;
    }
    let inline: String = value
        .split('#')
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    Some(if inline == "{workspace=true}" {
        Value::Inherited
    } else if starts_with_word(value, "true") {
        Value::Bool(true)
    } else if starts_with_word(value, "false") {
        Value::Bool(false)
    } else if let Some(text) = value
        .strip_prefix('"')
        .and_then(|rest| rest.split_once('"'))
    {
        Value::Text(text.0.to_owned())
    } else {
        Value::Absent
    })
}

/// Whether `value` is `word`, optionally followed by a comment.
fn starts_with_word(value: &str, word: &str) -> bool {
    value
        .strip_prefix(word)
        .is_some_and(|rest| rest.trim().is_empty() || rest.trim_start().starts_with('#'))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::findings;
    use crate::workspace;

    fn fixture_findings(case: &str) -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("license-headers")
            .join(case);
        findings(&root)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    const APACHE: &str = "must declare `license = \"Apache-2.0\"`, directly or through \
                          `license.workspace = true` (MG-REQ-1747)";
    const DUAL: &str = "may be published, so it must declare `license = \"MIT OR Apache-2.0\"`, \
                        directly or through `license.workspace = true` (MG-REQ-1747)";

    #[test]
    fn mg_req_1747_declared_licenses_pass() {
        assert_eq!(fixture_findings("valid"), Vec::<String>::new());
    }

    #[test]
    fn mg_req_1747_this_repository_passes() {
        let root = workspace::root().unwrap();
        assert!(findings(&root).unwrap().is_empty());
    }

    #[test]
    fn mg_req_1747_missing_or_wrong_licenses_fail() {
        assert_eq!(
            fixture_findings("broken"),
            [
                format!("crates/mergus-missing/Cargo.toml:1: license-headers: {APACHE}"),
                format!("crates/mergus-mit/Cargo.toml:1: license-headers: {APACHE}"),
                format!("crates/mergus-published/Cargo.toml:1: license-headers: {DUAL}"),
                format!("crates/mergus-unset/Cargo.toml:1: license-headers: {DUAL}"),
            ]
        );
    }

    #[test]
    fn mg_req_1747_inheriting_an_undeclared_workspace_license_fails() {
        assert_eq!(
            fixture_findings("no-workspace-license"),
            [format!("xtask/Cargo.toml:1: license-headers: {APACHE}")]
        );
    }
}
