//! The `size` check: Rust files over 400 lines and functions over 50 lines (MG-REQ-0178).

use super::lexer::{self, Kind, Token};
use super::{File, Finding, extension};

/// The most lines a file may have (MG-REQ-0178.1).
const MAX_FILE_LINES: usize = 400;

/// The most lines a function may have (MG-REQ-0178.2).
const MAX_FUNCTION_LINES: usize = 50;

/// How many lines at the top of a file may hold its waiver.
const FILE_WAIVER_LINES: usize = 5;

/// A function: its name and the lines of `fn` and of its closing brace, starting at 1.
struct Function {
    name: String,
    start: usize,
    end: usize,
}

/// Reports Rust files and functions that exceed the limits of MG-REQ-0012 without a waiver.
pub(super) fn findings(file: &File) -> Vec<Finding> {
    let Some(text) = &file.text else {
        return Vec::new();
    };
    if extension(&file.path) != Some("rs") {
        return Vec::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut findings = Vec::new();
    let file_waived = lines
        .iter()
        .take(FILE_WAIVER_LINES)
        .any(|line| is_waiver(line));
    if lines.len() > MAX_FILE_LINES && !file_waived {
        let message = format!(
            "file has {} lines, more than {MAX_FILE_LINES} (MG-REQ-0178.1)",
            lines.len()
        );
        findings.push(file.finding(1, "size", message));
    }
    for function in functions(&lexer::tokens(text)) {
        let count = function.end - function.start + 1;
        if count > MAX_FUNCTION_LINES && !function_waived(&lines, function.start) {
            let message = format!(
                "function `{}` has {count} lines, more than {MAX_FUNCTION_LINES} (MG-REQ-0178.2)",
                function.name
            );
            findings.push(file.finding(function.start, "size", message));
        }
    }
    findings
}

/// Finds every function with a body: the keyword `fn`, a name, and braces.
fn functions(tokens: &[Token]) -> Vec<Function> {
    let mut found = Vec::new();
    for (index, pair) in tokens.windows(2).enumerate() {
        let [keyword, name] = pair else {
            continue;
        };
        let (Kind::Word(keyword_text), Kind::Word(name_text)) = (&keyword.kind, &name.kind) else {
            continue;
        };
        if keyword_text != "fn" {
            continue;
        }
        if let Some(end) = body_end(tokens.get(index + 2..).unwrap_or_default()) {
            found.push(Function {
                name: name_text.clone(),
                start: keyword.line,
                end,
            });
        }
    }
    found
}

/// Returns the line of the brace that closes the body starting in `rest`, or `None` when
/// the function has no body, as in a trait.
fn body_end(rest: &[Token]) -> Option<usize> {
    let mut nesting = 0_usize;
    let mut braces = 0_usize;
    for token in rest {
        let Kind::Punct(punct) = token.kind else {
            continue;
        };
        match punct {
            '(' | '[' if braces == 0 => nesting += 1,
            ')' | ']' if braces == 0 => nesting = nesting.saturating_sub(1),
            ';' if braces == 0 && nesting == 0 => return None,
            '{' if nesting == 0 => braces += 1,
            '}' if nesting == 0 => {
                braces = braces.checked_sub(1)?;
                if braces == 0 {
                    return Some(token.line);
                }
            }
            _ => {}
        }
    }
    None
}

/// Returns whether the line directly above a function's attributes and documentation is a
/// waiver. `start` is the line of `fn`, starting at 1.
fn function_waived(lines: &[&str], start: usize) -> bool {
    let mut index = start.saturating_sub(1);
    while let Some(above) = index.checked_sub(1).and_then(|at| lines.get(at)) {
        let above = above.trim();
        if above.starts_with("///") || above.starts_with("#[") {
            index -= 1;
        } else if let Some(attribute) = attribute_start(lines, index - 1) {
            index = attribute;
        } else {
            return is_waiver(above);
        }
    }
    false
}

/// When the line at `end` closes an attribute written over several lines, returns the index
/// of the line where that attribute starts.
fn attribute_start(lines: &[&str], end: usize) -> Option<usize> {
    if !lines.get(end)?.trim_end().ends_with(']') {
        return None;
    }
    (0..end)
        .rev()
        .map_while(|at| {
            lines
                .get(at)
                .filter(|line| !line.trim().is_empty())
                .map(|line| (at, line))
        })
        .find(|(_, line)| line.trim_start().starts_with("#["))
        .map(|(at, _)| at)
}

/// Returns whether a line is the waiver `// size: <reason>` with a non-empty reason.
fn is_waiver(line: &str) -> bool {
    line.trim()
        .strip_prefix("// size:")
        .is_some_and(|reason| !reason.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::super::SIZE;
    use super::super::tests::{fixture_findings, repository_findings};

    #[test]
    fn mg_req_0178_1_file_over_400_lines_fails() {
        assert_eq!(
            fixture_findings(SIZE, "long-file"),
            ["long.rs:1: size: file has 401 lines, more than 400 (MG-REQ-0178.1)"]
        );
    }

    #[test]
    fn mg_req_0178_2_function_over_50_lines_fails() {
        assert_eq!(
            fixture_findings(SIZE, "long-function"),
            [
                "long.rs:4: size: function `too_long` has 51 lines, more than 50 (MG-REQ-0178.2)",
                "long.rs:62: size: function `inner` has 51 lines, more than 50 (MG-REQ-0178.2)",
                "tests.rs:3: size: function `long_test` has 51 lines, more than 50 (MG-REQ-0178.2)",
            ]
        );
    }

    #[test]
    fn mg_req_0178_items_at_the_limit_pass() {
        assert_eq!(fixture_findings(SIZE, "at-limit"), Vec::<String>::new());
    }

    #[test]
    fn mg_req_0178_waived_items_pass() {
        assert_eq!(fixture_findings(SIZE, "waived"), Vec::<String>::new());
    }

    #[test]
    fn mg_req_0178_empty_or_misplaced_waivers_fail() {
        assert_eq!(
            fixture_findings(SIZE, "bad-waiver"),
            [
                "empty.rs:1: size: file has 401 lines, more than 400 (MG-REQ-0178.1)",
                "late.rs:1: size: file has 401 lines, more than 400 (MG-REQ-0178.1)",
                "late.rs:9: size: function `separated` has 51 lines, more than 50 (MG-REQ-0178.2)",
            ]
        );
    }

    #[test]
    fn mg_req_0178_this_repository_passes() {
        assert_eq!(repository_findings(SIZE), Vec::<String>::new());
    }
}
