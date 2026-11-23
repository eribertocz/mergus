//! MG-REQ-1321, MG-REQ-1322, MG-REQ-1323, MG-REQ-1347: human output.

use mergus_sim::{SimEnvironment, SimFileSystem, SimRandom};
use serde_json::Value;

use super::{Outcome, PROJECT, ROOT, Setup, broken, project};
use crate::catalog;
use crate::mode::{Mode, colors, detect};

/// Runs `mergus <args>` in [`ROOT`] of `files`, standard output a terminal or not.
fn human(files: &SimFileSystem, args: &[&str], terminal: bool, vars: &[(&str, &str)]) -> Outcome {
    let random = SimRandom::new(7);
    Setup {
        files,
        cwd: ROOT,
        args,
        vars,
        random: &random,
        seed: 7,
        terminal,
    }
    .run()
}

fn has_colors(text: &str) -> bool {
    text.contains("\u{1b}[")
}

/// `text` without its ANSI escape sequences.
fn strip(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A project with two errors in `project.toml` and one in `models/`.
fn three_errors() -> SimFileSystem {
    project(&[
        ("project.toml", "name = \"sales\"\nnmae = \"x\"\nfoo = 1\n"),
        ("models/Orders.sql", "select 1"),
    ])
}

#[test]
fn mg_req_1321_output_mode_detection() {
    let files = broken();
    let terminal = human(&files, &["validate"], true, &[]);
    assert!(has_colors(&terminal.stderr), "{}", terminal.stderr);
    let piped = human(&files, &["validate"], false, &[]);
    assert!(!has_colors(&piped.stderr), "{}", piped.stderr);
    assert_eq!(strip(&terminal.stderr), piped.stderr);
    let no_color = human(&files, &["validate"], true, &[("NO_COLOR", "1")]);
    assert!(!has_colors(&no_color.stderr), "{}", no_color.stderr);
    let quiet = human(&files, &["validate", "--quiet"], false, &[]);
    assert_eq!(quiet.exit, piped.exit);
    assert_eq!(quiet.stderr, piped.stderr, "only errors");
    let verbose = human(&files, &["validate", "--verbose"], false, &[]);
    assert_eq!(verbose.exit, piped.exit);
    assert!(
        verbose.stderr.len() > piped.stderr.len(),
        "{}",
        verbose.stderr
    );
}

#[test]
fn mg_req_1322_error_layout() {
    let files = project(&[("project.toml", "name = \"sales\"\nnmae = \"x\"\n")]);
    let json = human(&files, &["validate", "--json"], false, &[]);
    let diagnostic = &json.of("diagnostic")[0];
    let message = diagnostic["message"].as_str().unwrap();
    let hint = diagnostic["hint"].as_str().unwrap();
    let why = catalog::entry("MG-0304").unwrap().cause;
    let outcome = human(&files, &["validate"], false, &[]);
    let expected = format!(
        "error[MG-0304]: {message}\n\
         \x20 --> project.toml:2\n\
         \x20  |\n\
         \x202 | nmae = \"x\"\n\
         \x20  | ^\n\
         \x20  = why: {why}\n\
         \x20  = fix: {hint}\n\
         \x20  = more: mergus explain MG-0304\n\n"
    );
    assert_eq!(outcome.stderr, expected);
    assert_eq!(outcome.stdout, "");
    assert_eq!(outcome.exit, 1);
    for word in ["your ", "wrong", "mistake", "fault"] {
        assert!(
            !outcome.stderr.to_lowercase().contains(word),
            "neutral: {word}"
        );
    }
}

#[test]
fn mg_req_1323_errors_grouped_by_file() {
    let outcome = human(&three_errors(), &["validate"], false, &[]);
    let arrows: Vec<&str> = outcome
        .stderr
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("--> "))
        .collect();
    assert_eq!(
        arrows,
        ["models/Orders.sql", "project.toml:2", "project.toml:3"],
        "{}",
        outcome.stderr
    );
    assert_eq!(outcome.stderr.matches("error[").count(), 3);
    interleaved_errors_are_grouped();
}

#[test]
fn mg_req_1347_modes_and_verbosity() {
    modes_and_verbosity_1();
    modes_and_verbosity_2();
    modes_and_verbosity_3();
    modes_and_verbosity_4();
}

/// One case of mode detection: `--json`, a terminal, the variables, the mode expected.
type ModeCase<'a> = (bool, bool, &'a [(&'a str, &'a str)], Mode);

/// MG-REQ-1347.1: interactive only on a terminal, without `--json`, `TERM` not `dumb`.
fn modes_and_verbosity_1() {
    let env = |vars: &[(&str, &str)]| SimEnvironment::new(vars.iter().copied(), None);
    let cases: [ModeCase<'_>; 6] = [
        (false, true, &[], Mode::Interactive),
        (
            false,
            true,
            &[("TERM", "xterm-256color")],
            Mode::Interactive,
        ),
        (false, true, &[("TERM", "dumb")], Mode::Plain),
        (false, false, &[], Mode::Plain),
        (true, true, &[], Mode::Events),
        (true, false, &[("TERM", "dumb")], Mode::Events),
    ];
    for (json, terminal, vars, mode) in cases {
        assert_eq!(
            detect(json, terminal, &env(vars)),
            mode,
            "{json} {terminal} {vars:?}"
        );
    }
    let dumb = human(&broken(), &["validate"], true, &[("TERM", "dumb")]);
    assert!(!has_colors(&dumb.stderr), "{}", dumb.stderr);
    let events = human(&broken(), &["validate", "--json"], true, &[]);
    assert!(!has_colors(&events.stdout) && events.stderr.is_empty());
}

/// MG-REQ-1347.2: a non-empty `NO_COLOR` removes colors and keeps everything else.
fn modes_and_verbosity_2() {
    let env = |vars: &[(&str, &str)]| SimEnvironment::new(vars.iter().copied(), None);
    assert!(colors(Mode::Interactive, &env(&[])));
    assert!(colors(Mode::Interactive, &env(&[("NO_COLOR", "")])));
    assert!(!colors(Mode::Interactive, &env(&[("NO_COLOR", "1")])));
    assert!(!colors(Mode::Plain, &env(&[])));
    let files = three_errors();
    let colored = human(&files, &["validate"], true, &[("NO_COLOR", "")]);
    assert!(has_colors(&colored.stderr));
    let plain = human(&files, &["validate"], true, &[("NO_COLOR", "1")]);
    assert_eq!(plain.stderr, strip(&colored.stderr));
    assert_eq!(plain.exit, colored.exit);
}

/// The events of `outcome` without the arguments of `run_started`.
fn stream(outcome: &Outcome) -> Vec<Value> {
    let mut events = outcome.events();
    for event in &mut events {
        if let Some(object) = event.as_object_mut() {
            object.remove("arguments");
        }
    }
    events
}

/// MG-REQ-1347.3: `--quiet` shows only errors, `--verbose` adds a line per stage with its
/// duration, and neither changes the events.
fn modes_and_verbosity_3() {
    let files = broken();
    let normal = human(&files, &["validate"], false, &[]);
    let quiet = human(&files, &["validate", "--quiet"], false, &[]);
    assert_eq!(quiet.stderr, normal.stderr);
    assert!(quiet.stderr.lines().all(|line| !line.starts_with("stage")));
    let verbose = human(&files, &["validate", "--verbose"], false, &[]);
    let stages: Vec<&str> = verbose
        .stderr
        .lines()
        .filter(|line| line.starts_with("stage"))
        .collect();
    assert_eq!(stages.len(), 1, "{}", verbose.stderr);
    assert!(stages[0].ends_with(" ms"), "{}", stages[0]);
    assert_eq!(
        verbose.stderr.replace(&format!("{}\n", stages[0]), ""),
        normal.stderr
    );
    let clean = project(&[("project.toml", PROJECT)]);
    assert_eq!(
        human(&clean, &["validate", "--quiet"], false, &[]).stderr,
        ""
    );
    let events = human(&files, &["validate", "--json"], false, &[]);
    for flag in ["--quiet", "--verbose"] {
        let with = human(&files, &["validate", "--json", flag], false, &[]);
        assert_eq!(stream(&with), stream(&events), "{flag}");
        assert!(with.stderr.is_empty(), "{flag}: {}", with.stderr);
    }
}

/// MG-REQ-1347.4: `--quiet` with `--verbose` is an invalid command line.
fn modes_and_verbosity_4() {
    let clean = project(&[("project.toml", PROJECT)]);
    for args in [
        &["validate", "--quiet", "--verbose", "--json"][..],
        &["--verbose", "validate", "--quiet", "--json"][..],
    ] {
        let outcome = human(&clean, args, false, &[]);
        assert_eq!(outcome.exit, 1, "{args:?}");
        let diagnostics = outcome.of("diagnostic");
        assert_eq!(diagnostics.len(), 1, "{outcome:?}");
        assert_eq!(diagnostics[0]["code"], "MG-1301");
        let hint = diagnostics[0]["hint"].as_str().unwrap();
        assert!(
            hint.contains("`--quiet`") && hint.contains("`--verbose`"),
            "{hint}"
        );
        assert!(outcome.of("run_started").is_empty());
    }
    let human_run = human(&clean, &["validate", "--quiet", "--verbose"], false, &[]);
    assert_eq!(human_run.exit, 1);
    assert!(
        human_run.stderr.starts_with("error[MG-1301]"),
        "{}",
        human_run.stderr
    );
}

/// Errors of several files, interleaved, come out grouped by file in order of first
/// appearance; errors without a file come last.
fn interleaved_errors_are_grouped() {
    use crate::diagnostic::{Diagnostic, Location, group};
    let of = |file: Option<&str>, message: &str| Diagnostic {
        code: "MG-0304",
        message: message.to_owned(),
        location: file.map(|file| Location {
            file: file.to_owned(),
            line: Some(1),
            column: None,
        }),
        hint: None,
        next: Vec::new(),
        source: None,
    };
    let grouped = group(vec![
        of(Some("b.toml"), "1"),
        of(None, "2"),
        of(Some("a.toml"), "3"),
        of(Some("b.toml"), "4"),
        of(Some("a.toml"), "5"),
    ]);
    let order: Vec<&str> = grouped.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(order, ["1", "4", "3", "5", "2"]);
}
