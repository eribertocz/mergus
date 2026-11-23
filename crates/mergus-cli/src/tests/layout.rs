//! MG-REQ-1346: where each line of an error comes from.

use crate::catalog;
use crate::diagnostic::{Diagnostic, Location};
use crate::layout::render;

fn diagnostic(code: &'static str, location: Option<Location>, source: Option<&str>) -> Diagnostic {
    Diagnostic {
        code,
        message: "key `nmae` is not allowed".to_owned(),
        location,
        hint: None,
        next: Vec::new(),
        source: source.map(str::to_owned),
    }
}

fn at(file: &str, line: Option<usize>, column: Option<usize>) -> Option<Location> {
    Some(Location {
        file: file.to_owned(),
        line,
        column,
    })
}

fn text(diagnostic: &Diagnostic) -> String {
    render(&diagnostic.resolve().unwrap(), false)
}

#[test]
fn mg_req_1346_error_lines_sources() {
    error_lines_sources_1();
    error_lines_sources_2();
    error_lines_sources_3();
    error_lines_sources_4();
    error_lines_sources_5();
}

/// MG-REQ-1346.1: the title is the message, `-->` the location, the excerpt only with a line.
fn error_lines_sources_1() {
    let with_line = text(&diagnostic(
        "MG-0304",
        at("project.toml", Some(12), Some(3)),
        Some("a.nmae = 1"),
    ));
    let lines: Vec<&str> = with_line.lines().collect();
    assert_eq!(lines[0], "error[MG-0304]: key `nmae` is not allowed");
    assert_eq!(lines[1], "  --> project.toml:12:3");
    assert_eq!(lines[2], "   |");
    assert_eq!(lines[3], "12 | a.nmae = 1");
    let line_only = text(&diagnostic(
        "MG-0304",
        at("project.toml", Some(2), None),
        Some("x"),
    ));
    assert!(line_only.contains("  --> project.toml:2\n"), "{line_only}");
    let no_line = text(&diagnostic(
        "MG-0302",
        at("models/Orders.sql", None, None),
        None,
    ));
    assert!(no_line.contains("  --> models/Orders.sql\n"), "{no_line}");
    assert!(
        !no_line.contains(" |"),
        "no excerpt without a line: {no_line}"
    );
    let nowhere = text(&diagnostic("MG-1302", None, None));
    assert!(
        !nowhere.contains("-->") && !nowhere.contains(" |"),
        "{nowhere}"
    );
    assert!(nowhere.starts_with("error[MG-1302]: key `nmae` is not allowed\n"));
}

/// MG-REQ-1346.2: one `^` at the column, or under the first character without one.
fn error_lines_sources_2() {
    let source = Some("a.nmae = 1");
    let column = text(&diagnostic(
        "MG-0304",
        at("project.toml", Some(2), Some(3)),
        source,
    ));
    assert!(column.contains("\n 2 | a.nmae = 1\n   |   ^\n"), "{column}");
    let first = text(&diagnostic(
        "MG-0304",
        at("project.toml", Some(2), None),
        source,
    ));
    assert!(first.contains("\n 2 | a.nmae = 1\n   | ^\n"), "{first}");
    let tab = text(&diagnostic(
        "MG-0304",
        at("project.toml", Some(2), Some(3)),
        Some("\tx = 1"),
    ));
    assert!(tab.contains("\n   | \t ^\n"), "a tab stays a tab: {tab:?}");
    assert_eq!(column.matches('^').count(), 1);
}

/// MG-REQ-1346.3: `why` is the first paragraph of `## Cause` of the catalog file.
fn error_lines_sources_3() {
    let why = "Two files under `models/` have the same stem. Model names are unique in the \
               project, whatever their folder.";
    assert_eq!(catalog::entry("MG-0301").unwrap().cause, why);
    let shown = text(&diagnostic(
        "MG-0301",
        at("models/b/a.sql", None, None),
        None,
    ));
    assert!(shown.contains(&format!("\n   = why: {why}\n")), "{shown}");
    for entry in catalog::entries() {
        let entry = entry.unwrap();
        assert!(
            !entry.cause.is_empty() && !entry.cause.contains('\n'),
            "{}",
            entry.code
        );
    }
}

/// MG-REQ-1346.4: `fix` is the diagnostic's hint.
fn error_lines_sources_4() {
    let mut precise = diagnostic("MG-1301", None, None);
    precise.hint = Some("Use `validate`.".to_owned());
    assert!(text(&precise).contains("\n   = fix: Use `validate`.\n"));
    let catalog_hint = catalog::entry("MG-1302").unwrap().hint;
    let generic = text(&diagnostic("MG-1302", None, None));
    assert!(
        generic.contains(&format!("\n   = fix: {catalog_hint}\n")),
        "{generic}"
    );
}

/// MG-REQ-1346.5: `more` is always `mergus explain <code>`.
fn error_lines_sources_5() {
    for entry in catalog::entries() {
        let code = entry.unwrap().code;
        for location in [None, at("project.toml", Some(1), None)] {
            let shown = text(&diagnostic(code, location, Some("x = 1")));
            let last = shown.trim_end().lines().last().unwrap();
            assert_eq!(last, format!("   = more: mergus explain {code}"));
        }
    }
}
