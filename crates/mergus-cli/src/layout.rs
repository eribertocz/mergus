//! The layout of an error in human output (MG-REQ-1322, MG-REQ-1346).

use std::fmt::Write;

use crate::diagnostic::Resolved;
use crate::name::PRODUCT;

/// The ANSI sequences of the colored layout.
const RED: &str = "\u{1b}[1;31m";
const YELLOW: &str = "\u{1b}[1;33m";
const BLUE: &str = "\u{1b}[1;34m";
const BOLD: &str = "\u{1b}[1m";
const RESET: &str = "\u{1b}[0m";

/// Paints text when colors are on.
#[derive(Clone, Copy)]
struct Paint(bool);

impl Paint {
    fn on(self, style: &str, text: &str) -> String {
        if self.0 {
            format!("{style}{text}{RESET}")
        } else {
            text.to_owned()
        }
    }
}

/// Renders `resolved` as a block of lines that ends with an empty line: title, location,
/// source excerpt, then `why`, `fix` and `more`. A warning is titled `warning` in yellow.
pub(crate) fn render(resolved: &Resolved<'_>, color: bool) -> String {
    let paint = Paint(color);
    let diagnostic = resolved.diagnostic;
    let location = diagnostic.location.as_ref();
    let line = location.and_then(|at| at.line);
    let width = line.map_or(0, |line| line.to_string().len()).max(2);
    let pad = " ".repeat(width);
    let bar = paint.on(BLUE, &format!("{pad} |"));
    let (title, accent) = if diagnostic.warning {
        ("warning", YELLOW)
    } else {
        ("error", RED)
    };
    let mut text = format!(
        "{}{}\n",
        paint.on(accent, &format!("{title}[{}]", diagnostic.code)),
        paint.on(BOLD, &format!(": {}", diagnostic.message)),
    );
    if let Some(at) = location {
        let mut place = at.file.clone();
        for number in [at.line, at.column].into_iter().flatten() {
            let _ = write!(place, ":{number}");
        }
        let _ = writeln!(text, "{pad}{} {place}", paint.on(BLUE, "-->"));
        if let (Some(line), Some(source)) = (line, diagnostic.source.as_deref()) {
            let number = paint.on(BLUE, &format!("{line:>width$} |"));
            let _ = writeln!(text, "{bar}\n{number} {source}");
            let indent = underline_indent(source, at.column);
            let _ = writeln!(text, "{bar} {indent}{}", paint.on(accent, "^"));
        }
    }
    let equals = paint.on(BLUE, &format!("{pad} ="));
    let _ = writeln!(text, "{equals} why: {}", resolved.why);
    let _ = writeln!(text, "{equals} fix: {}", resolved.hint);
    let _ = writeln!(text, "{equals} more: {PRODUCT} explain {}", diagnostic.code);
    text.push('\n');
    text
}

/// The blanks before the `^` under `column` of `source`, from 1: a tab stays a tab so that
/// the `^` lines up; no column puts it under the first character.
fn underline_indent(source: &str, column: Option<usize>) -> String {
    let before = column.unwrap_or(1).saturating_sub(1);
    source
        .chars()
        .chain(std::iter::repeat(' '))
        .take(before)
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect()
}
