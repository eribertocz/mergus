//! The `sql-concatenation` check: SQL text built with formatting macros (MG-REQ-0184).

use super::lexer::{self, Kind, Token};
use super::{File, Finding, extension};

/// The formatting macros whose literals are checked.
const MACROS: [&str; 4] = ["format", "write", "writeln", "concat"];

/// The SQL keywords a checked literal starts with.
const KEYWORDS: [&str; 11] = [
    "SELECT", "INSERT", "UPDATE", "DELETE", "MERGE", "CREATE", "DROP", "ALTER", "WITH", "COPY",
    "ATTACH",
];

/// The start of the waiver comment; the name of the quoting function follows it.
const WAIVER: &str = "// sql: identifiers quoted by ";

/// Reports every SQL literal with a `{` placeholder passed to a formatting macro.
pub(super) fn findings(file: &File) -> Vec<Finding> {
    let Some(text) = &file.text else {
        return Vec::new();
    };
    if extension(&file.path) != Some("rs") {
        return Vec::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let tokens = lexer::tokens(text);
    let mut findings = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let Kind::Word(word) = &token.kind else {
            continue;
        };
        let is_macro = matches!(
            tokens.get(index + 1),
            Some(Token {
                kind: Kind::Punct('!'),
                ..
            })
        );
        if !MACROS.contains(&word.as_str()) || !is_macro {
            continue;
        }
        for (literal, line) in arguments(tokens.get(index + 2..).unwrap_or_default()) {
            if is_formatted_sql(literal) && !waived(&lines, line) {
                let message = "SQL literal with a `{` placeholder: bind values as parameters \
                               (MG-REQ-0184)";
                findings.push(file.finding(line, "sql-concatenation", message.to_owned()));
            }
        }
    }
    findings
}

/// Returns the string literals passed directly to a macro whose opening delimiter starts
/// `rest`, with their lines.
fn arguments(rest: &[Token]) -> Vec<(&str, usize)> {
    let mut literals = Vec::new();
    let mut depth = 0_usize;
    for token in rest {
        match &token.kind {
            Kind::Punct('(' | '[' | '{') => depth += 1,
            Kind::Punct(')' | ']' | '}') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Kind::Str(text) if depth == 1 => literals.push((text.as_str(), token.line)),
            _ if depth == 0 => break,
            _ => {}
        }
    }
    literals
}

/// Returns whether a literal starts with an SQL keyword, ignoring case, and holds a
/// placeholder: a `{` that is not the escaped brace `{{`.
fn is_formatted_sql(literal: &str) -> bool {
    let start = literal.trim_start();
    let is_sql = KEYWORDS.iter().any(|keyword| {
        start
            .get(..keyword.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(keyword))
    });
    is_sql && literal.replace("{{", "").contains('{')
}

/// Returns whether the line directly above `line`, starting at 1, is the waiver naming a
/// quoting function.
fn waived(lines: &[&str], line: usize) -> bool {
    line.checked_sub(2)
        .and_then(|above| lines.get(above))
        .and_then(|above| above.trim().strip_prefix(WAIVER))
        .is_some_and(|function| !function.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::super::SQL_CONCATENATION;
    use super::super::tests::{fixture_findings, repository_findings};

    const MESSAGE: &str = "sql-concatenation: SQL literal with a `{` placeholder: bind values as parameters (MG-REQ-0184)";

    #[test]
    fn mg_req_0184_formatted_sql_fails() {
        let expected: Vec<String> = [3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 16, 20]
            .iter()
            .map(|line| format!("lib.rs:{line}: {MESSAGE}"))
            .collect();
        assert_eq!(fixture_findings(SQL_CONCATENATION, "formatted"), expected);
    }

    #[test]
    fn mg_req_0184_waived_and_unformatted_sql_pass() {
        assert_eq!(
            fixture_findings(SQL_CONCATENATION, "clean"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn mg_req_0184_waiver_without_function_fails() {
        assert_eq!(
            fixture_findings(SQL_CONCATENATION, "bad-waiver"),
            [
                format!("lib.rs:3: {MESSAGE}"),
                format!("lib.rs:7: {MESSAGE}"),
            ]
        );
    }

    #[test]
    fn mg_req_0184_this_repository_passes() {
        assert_eq!(repository_findings(SQL_CONCATENATION), Vec::<String>::new());
    }
}
