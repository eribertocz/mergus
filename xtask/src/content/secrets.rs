//! The `secrets` check: secrets, secret files, build artifacts, data files and
//! commented-out code (MG-REQ-0179).

use super::lexer;
use super::{File, Finding, extension};

/// Extensions of secret files (MG-REQ-0179.2).
const SECRET_EXTENSIONS: [&str; 4] = ["pem", "key", "p12", "pfx"];

/// Extensions of build artifacts (MG-REQ-0179.3).
const ARTIFACT_EXTENSIONS: [&str; 10] = [
    "rlib", "rmeta", "so", "dylib", "dll", "exe", "o", "a", "pyc", "whl",
];

/// Extensions of data files (MG-REQ-0179.4).
const DATA_EXTENSIONS: [&str; 10] = [
    "parquet", "csv", "tsv", "arrow", "feather", "avro", "duckdb", "db", "sqlite", "xlsx",
];

/// A secret pattern of MG-REQ-0179.1: its text as the rule writes it, and its matcher.
struct Pattern {
    text: &'static str,
    matches: fn(&str) -> bool,
}

/// The secret patterns of MG-REQ-0179.1, in the order of the rule.
const PATTERNS: [Pattern; 5] = [
    Pattern {
        text: "-----BEGIN [A-Z ]*PRIVATE KEY-----",
        matches: private_key,
    },
    Pattern {
        text: "AKIA[0-9A-Z]{16}",
        matches: aws_key,
    },
    Pattern {
        text: "gh[pousr]_[A-Za-z0-9]{36,}",
        matches: github_token,
    },
    Pattern {
        text: "github_pat_[A-Za-z0-9_]{22,}",
        matches: github_pat,
    },
    Pattern {
        text: "xox[abprs]-[A-Za-z0-9-]{10,}",
        matches: slack_token,
    },
];

/// Reports every item of MG-REQ-0179 found in a file or its path.
pub(super) fn findings(file: &File) -> Vec<Finding> {
    let mut findings = Vec::new();
    if let Some(message) = path_problem(&file.path) {
        findings.push(file.finding(1, "secrets", message.to_owned()));
    }
    let Some(text) = &file.text else {
        return findings;
    };
    let comments = if extension(&file.path) == Some("rs") {
        lexer::line_comments(text)
    } else {
        Vec::new()
    };
    for (line, number) in text.lines().zip(1..) {
        if let Some(pattern) = PATTERNS.iter().find(|pattern| (pattern.matches)(line)) {
            let message = format!("line matches `{}` (MG-REQ-0179.1)", pattern.text);
            findings.push(file.finding(number, "secrets", message));
        }
        if comments.contains(&number) && is_commented_out_code(line) {
            let message = "commented-out code (MG-REQ-0179.5)".to_owned();
            findings.push(file.finding(number, "secrets", message));
        }
    }
    findings
}

/// Returns the problem of a path: a secret file, a build artifact or a data file.
fn path_problem(path: &str) -> Option<&'static str> {
    let parts: Vec<&str> = path.split('/').collect();
    let (name, dirs) = parts.split_last()?;
    let extension = extension(path).unwrap_or_default();
    if *name == ".env" || name.starts_with(".env.") || SECRET_EXTENSIONS.contains(&extension) {
        return Some("secret file (MG-REQ-0179.2)");
    }
    if dirs.contains(&"target") || ARTIFACT_EXTENSIONS.contains(&extension) {
        return Some("build artifact (MG-REQ-0179.3)");
    }
    if !dirs.contains(&"tests") && DATA_EXTENSIONS.contains(&extension) {
        return Some("data file outside a `tests/` directory (MG-REQ-0179.4)");
    }
    None
}

/// Returns whether a Rust line that holds a `//` comment starts with it, is not
/// documentation, and ends like code (MG-REQ-0179.5).
fn is_commented_out_code(line: &str) -> bool {
    let line = line.trim();
    if !line.starts_with("//") || line.starts_with("///") || line.starts_with("//!") {
        return false;
    }
    line.ends_with(';') || line.ends_with('{') || line.ends_with('}')
}

/// Returns the length of the run at the start of `text` of characters accepted by `class`.
fn run(text: &str, class: fn(char) -> bool) -> usize {
    text.chars().take_while(|&c| class(c)).count()
}

/// Returns whether some occurrence of `prefix` in `line` is followed by a run of at least
/// `min` characters of `class`.
fn prefixed_run(line: &str, prefix: &str, class: fn(char) -> bool, min: usize) -> bool {
    line.match_indices(prefix)
        .any(|(at, _)| run(&line[at + prefix.len()..], class) >= min)
}

/// `-----BEGIN [A-Z ]*PRIVATE KEY-----`
fn private_key(line: &str) -> bool {
    const PREFIX: &str = "-----BEGIN ";
    line.match_indices(PREFIX).any(|(at, _)| {
        let rest = &line[at + PREFIX.len()..];
        let span = run(rest, |c| c.is_ascii_uppercase() || c == ' ');
        (0..=span).any(|skip| rest[skip..].starts_with("PRIVATE KEY-----"))
    })
}

/// `AKIA[0-9A-Z]{16}`
fn aws_key(line: &str) -> bool {
    prefixed_run(
        line,
        "AKIA",
        |c| c.is_ascii_digit() || c.is_ascii_uppercase(),
        16,
    )
}

/// `gh[pousr]_[A-Za-z0-9]{36,}`
fn github_token(line: &str) -> bool {
    ['p', 'o', 'u', 's', 'r'].iter().any(|kind| {
        prefixed_run(
            line,
            &format!("gh{kind}_"),
            |c| c.is_ascii_alphanumeric(),
            36,
        )
    })
}

/// `github_pat_[A-Za-z0-9_]{22,}`
fn github_pat(line: &str) -> bool {
    prefixed_run(
        line,
        "github_pat_",
        |c| c.is_ascii_alphanumeric() || c == '_',
        22,
    )
}

/// `xox[abprs]-[A-Za-z0-9-]{10,}`
fn slack_token(line: &str) -> bool {
    ['a', 'b', 'p', 'r', 's'].iter().any(|kind| {
        prefixed_run(
            line,
            &format!("xox{kind}-"),
            |c| c.is_ascii_alphanumeric() || c == '-',
            10,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::super::SECRETS;
    use super::super::tests::{fixture_findings, repository_findings};

    #[test]
    fn mg_req_0179_1_secret_patterns_fail() {
        assert_eq!(
            fixture_findings(SECRETS, "patterns"),
            [
                "config.txt:1: secrets: line matches `-----BEGIN [A-Z ]*PRIVATE KEY-----` (MG-REQ-0179.1)",
                "config.txt:2: secrets: line matches `-----BEGIN [A-Z ]*PRIVATE KEY-----` (MG-REQ-0179.1)",
                "config.txt:3: secrets: line matches `AKIA[0-9A-Z]{16}` (MG-REQ-0179.1)",
                "config.txt:4: secrets: line matches `gh[pousr]_[A-Za-z0-9]{36,}` (MG-REQ-0179.1)",
                "config.txt:5: secrets: line matches `gh[pousr]_[A-Za-z0-9]{36,}` (MG-REQ-0179.1)",
                "config.txt:6: secrets: line matches `github_pat_[A-Za-z0-9_]{22,}` (MG-REQ-0179.1)",
                "config.txt:7: secrets: line matches `xox[abprs]-[A-Za-z0-9-]{10,}` (MG-REQ-0179.1)",
            ]
        );
    }

    #[test]
    fn mg_req_0179_2_secret_files_fail() {
        assert_eq!(
            fixture_findings(SECRETS, "secret-files"),
            [
                ".env:1: secrets: secret file (MG-REQ-0179.2)",
                ".env.local:1: secrets: secret file (MG-REQ-0179.2)",
                "certs/client.p12:1: secrets: secret file (MG-REQ-0179.2)",
                "certs/client.pfx:1: secrets: secret file (MG-REQ-0179.2)",
                "certs/server.key:1: secrets: secret file (MG-REQ-0179.2)",
                "certs/server.pem:1: secrets: secret file (MG-REQ-0179.2)",
                "config/.env:1: secrets: secret file (MG-REQ-0179.2)",
            ]
        );
    }

    #[test]
    fn mg_req_0179_3_build_artifacts_fail() {
        assert_eq!(
            fixture_findings(SECRETS, "artifacts"),
            [
                "crate/target/notes.txt:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.a:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.dll:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.dylib:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.exe:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.o:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.pyc:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.rlib:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.rmeta:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.so:1: secrets: build artifact (MG-REQ-0179.3)",
                "lib/a.whl:1: secrets: build artifact (MG-REQ-0179.3)",
                "target/out.txt:1: secrets: build artifact (MG-REQ-0179.3)",
            ]
        );
    }

    #[test]
    fn mg_req_0179_4_data_files_outside_tests_fail() {
        assert_eq!(
            fixture_findings(SECRETS, "data-files"),
            [
                "data/a.arrow:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.avro:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.csv:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.db:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.duckdb:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.feather:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.parquet:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.sqlite:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.tsv:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
                "data/a.xlsx:1: secrets: data file outside a `tests/` directory (MG-REQ-0179.4)",
            ]
        );
    }

    #[test]
    fn mg_req_0179_5_commented_out_code_fails() {
        assert_eq!(
            fixture_findings(SECRETS, "commented-code"),
            [
                "lib.rs:3: secrets: commented-out code (MG-REQ-0179.5)",
                "lib.rs:4: secrets: commented-out code (MG-REQ-0179.5)",
                "lib.rs:5: secrets: commented-out code (MG-REQ-0179.5)",
            ]
        );
    }

    #[test]
    fn mg_req_0179_clean_files_pass() {
        assert_eq!(fixture_findings(SECRETS, "clean"), Vec::<String>::new());
    }

    #[test]
    fn mg_req_0179_this_repository_passes() {
        assert_eq!(repository_findings(SECRETS), Vec::<String>::new());
    }
}
