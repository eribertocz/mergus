//! MG-REQ-0008, MG-REQ-0297, MG-REQ-1324, MG-REQ-1334: typed, structured and actionable
//! diagnostics.

use mergus_project::ProjectError;
use serde_json::Value;

use super::{PROJECT, broken, mergus, project};
use crate::catalog;
use crate::error::CliError;

fn is_code(text: &str) -> bool {
    text.strip_prefix("MG-")
        .is_some_and(|digits| digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()))
}

#[test]
fn mg_req_0008_every_error_has_code() {
    cli_errors_have_codes();
    scaffolding_errors_have_codes();
    project_codes_are_in_the_catalog();
    emitted_diagnostics_have_codes();
}

fn cli_errors_have_codes() {
    let errors = [
        CliError::UnknownCommand {
            given: "valdate".to_owned(),
            closest: "validate",
        },
        CliError::UnknownOption {
            given: "--jsn".to_owned(),
            closest: "--json",
        },
        CliError::UnexpectedArgument {
            given: "x".to_owned(),
            command: "validate",
        },
        CliError::MissingCommand {
            closest: "validate",
        },
        CliError::ProjectNotFound {
            start: "/tmp".to_owned(),
        },
        CliError::Unreadable("`models` was not found".to_owned()),
        CliError::Internal("broken".to_owned()),
    ];
    let codes: Vec<&str> = errors.iter().map(CliError::code).collect();
    assert_eq!(
        codes,
        [
            "MG-1301", "MG-1301", "MG-1301", "MG-1301", "MG-1302", "MG-1303", "MG-1304"
        ]
    );
    for error in &errors {
        assert!(catalog::entry(error.code()).is_some(), "{}", error.code());
        assert!(!error.to_string().is_empty());
    }
}

fn scaffolding_errors_have_codes() {
    let errors = [
        CliError::InvalidName {
            kind: "model",
            name: "Orders".to_owned(),
            suggestion: "orders".to_owned(),
        },
        CliError::NameTaken {
            name: "orders".to_owned(),
            path: "models/staging/orders.sql".to_owned(),
        },
        CliError::FileExists {
            path: "project.toml".to_owned(),
        },
        CliError::Unwritable {
            path: "models".to_owned(),
            message: "read only".to_owned(),
        },
    ];
    let codes: Vec<&str> = errors.iter().map(CliError::code).collect();
    assert_eq!(codes, ["MG-1305", "MG-1306", "MG-1307", "MG-1308"]);
    for error in &errors {
        assert!(catalog::entry(error.code()).is_some(), "{}", error.code());
    }
}

fn project_codes_are_in_the_catalog() {
    let project_error = ProjectError::DuplicateModel {
        name: "a".to_owned(),
        first: "models/a.sql".to_owned(),
        second: "models/b/a.sql".to_owned(),
    };
    assert_eq!(project_error.code(), "MG-0301");
    for code in [
        "MG-0301", "MG-0302", "MG-0303", "MG-0304", "MG-0305", "MG-0306", "MG-0307", "MG-0308",
        "MG-0309",
    ] {
        assert!(catalog::entry(code).is_some(), "{code} has a catalog entry");
    }
}

fn emitted_diagnostics_have_codes() {
    let outcome = mergus(&broken(), &["validate", "--json"]);
    let diagnostics = outcome.of("diagnostic");
    assert_eq!(diagnostics.len(), 2, "{outcome:?}");
    for diagnostic in &diagnostics {
        assert!(
            is_code(diagnostic["code"].as_str().unwrap()),
            "{diagnostic}"
        );
        assert!(!diagnostic["message"].as_str().unwrap().is_empty());
    }
    assert_eq!(diagnostics[1]["location"]["file"], "project.toml");
    assert_eq!(diagnostics[1]["location"]["line"], 2);
}

/// The single diagnostic and the exit code of `mergus <args>`.
fn invalid(args: &[&str]) -> (Value, u8, Vec<Value>) {
    let outcome = mergus(&project(&[("project.toml", PROJECT)]), args);
    let diagnostics = outcome.of("diagnostic");
    assert_eq!(diagnostics.len(), 1, "{outcome:?}");
    (diagnostics[0].clone(), outcome.exit, outcome.events())
}

#[test]
fn mg_req_0297_invalid_command_line() {
    let cases: [(&[&str], &str); 5] = [
        (&["valdate", "--json"], "`validate`"),
        (&["--json", "validate", "--jsno"], "`--json`"),
        (&["validate", "--json=yes", "--json"], "`--json`"),
        (&["--json"], "`validate`"),
        (&["validate", "extra", "--json"], "`extra`"),
    ];
    for (args, word) in cases {
        let (diagnostic, exit, events) = invalid(args);
        assert_eq!(exit, 1, "{args:?}");
        assert_eq!(diagnostic["code"], "MG-1301");
        assert!(
            diagnostic["hint"].as_str().unwrap().contains(word),
            "{diagnostic}"
        );
        let kinds: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(
            kinds,
            ["diagnostic", "run_finished"],
            "no run_started: {args:?}"
        );
    }
    let human = mergus(&project(&[("project.toml", PROJECT)]), &["valdate"]);
    assert_eq!(human.exit, 1);
    assert_eq!(human.stdout, "");
    assert!(human.stderr.contains("`validate`"), "{}", human.stderr);
}

#[test]
fn mg_req_1324_diagnostics_structured() {
    let outcome = mergus(&broken(), &["validate", "--json"]);
    let diagnostics = outcome.of("diagnostic");
    assert_eq!(diagnostics[0]["code"], "MG-0302");
    assert_eq!(diagnostics[0]["severity"], "error");
    assert!(
        diagnostics[0].get("location").is_none(),
        "a directory entry has no line"
    );
    assert!(
        diagnostics[0]["message"]
            .as_str()
            .unwrap()
            .contains("`orders`"),
        "the suggestion is in the message: {}",
        diagnostics[0]
    );
    assert_eq!(diagnostics[1]["code"], "MG-0304");
    assert_eq!(diagnostics[1]["severity"], "error");
    assert_eq!(diagnostics[1]["location"]["file"], "project.toml");
    assert_eq!(diagnostics[1]["location"]["line"], 2);
    assert!(
        diagnostics[1]["message"]
            .as_str()
            .unwrap()
            .contains("`name`")
    );
    let human = mergus(&broken(), &["validate"]);
    assert!(human.stderr.contains("MG-0302") && human.stderr.contains("MG-0304"));
    assert!(human.stderr.contains("project.toml:2"), "{}", human.stderr);
}

#[test]
fn mg_req_1334_diagnostics_are_actionable() {
    let secret = project(&[(
        "project.toml",
        "name = \"sales\"\n\n[gold]\nlocation = \"s3://key:hunter2@bucket/gold\"\n",
    )]);
    let outcomes = [
        mergus(&broken(), &["validate", "--json"]),
        mergus(&secret, &["validate", "--json"]),
        mergus(&project(&[]), &["validate", "--json"]),
        mergus(
            &project(&[("project.toml", PROJECT)]),
            &["valdate", "--json"],
        ),
    ];
    for outcome in &outcomes {
        assert!(!outcome.stdout.contains("hunter2"), "{}", outcome.stdout);
        for diagnostic in outcome.of("diagnostic") {
            let hint = diagnostic["hint"].as_str().unwrap();
            assert!(hint.ends_with('.'), "{hint}");
            assert_eq!(hint.matches(". ").count(), 0, "one sentence: {hint}");
            if let Some(next) = diagnostic.get("next") {
                for invocation in next.as_array().unwrap() {
                    assert_eq!(invocation[0], "mergus", "{invocation}");
                }
            }
        }
    }
    let again = mergus(&broken(), &["validate", "--json"]);
    assert_eq!(again.stdout, outcomes[0].stdout, "deterministic templates");
    let not_found = &outcomes[2].of("diagnostic")[0];
    assert_eq!(not_found["next"], serde_json::json!([["mergus", "init"]]));
}
