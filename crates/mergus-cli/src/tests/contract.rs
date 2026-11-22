//! MG-REQ-0260, MG-REQ-0261, MG-REQ-0262, MG-REQ-0264: the `--json` output and exit codes.

use mergus_contracts::generated::mergus_events_v1::MergusEvents;
use mergus_contracts::validate_event;
use mergus_sim::SimRandom;

use super::{Outcome, PROJECT, ROOT, Setup, broken, mergus, project};
use crate::catalog;
use crate::exit;

/// Runs that cover success, project errors and an invalid command line.
fn outcomes() -> Vec<Outcome> {
    let valid = project(&[("project.toml", PROJECT), ("models/orders.sql", "select 1")]);
    vec![
        mergus(&valid, &["validate", "--json"]),
        mergus(&broken(), &["--json", "validate"]),
        mergus(&valid, &["valdate", "--json"]),
        mergus(&project(&[]), &["validate", "--json"]),
    ]
}

#[test]
fn mg_req_0260_json_output_is_valid_event_stream() {
    for outcome in outcomes() {
        let events = outcome.events();
        assert!(!events.is_empty(), "{outcome:?}");
        for (index, value) in events.iter().enumerate() {
            let event: MergusEvents = serde_json::from_value(value.clone()).unwrap();
            validate_event(&event).unwrap();
            assert_eq!(event.sequence, i64::try_from(index).unwrap() + 1);
        }
        assert_eq!(outcome.of("run_finished").len(), 1, "{outcome:?}");
        assert_eq!(events.last().unwrap()["type"], "run_finished");
    }
}

#[test]
fn mg_req_0261_json_stdout_contains_only_events() {
    for outcome in outcomes() {
        assert!(outcome.stdout.ends_with('\n'), "{outcome:?}");
        for line in outcome.stdout.lines() {
            let event: MergusEvents = serde_json::from_str(line).unwrap();
            validate_event(&event).unwrap();
        }
    }
    let without = mergus(&broken(), &["validate"]);
    assert_eq!(without.stdout, "", "human diagnostics go to standard error");
    assert!(without.stderr.contains("MG-0302"), "{}", without.stderr);
}

#[test]
fn mg_req_0262_exit_code_matches_table() {
    let expected = [0, 1, 1, 1];
    for (outcome, code) in outcomes().into_iter().zip(expected) {
        assert_eq!(outcome.exit, code, "{outcome:?}");
        let finished = &outcome.of("run_finished")[0];
        assert_eq!(finished["exit_code"], u64::from(code));
        assert_eq!(finished["retryable"], false);
    }
    let table = [
        (0, Some(false)),
        (1, Some(false)),
        (2, Some(false)),
        (3, Some(true)),
        (4, Some(true)),
        (5, Some(false)),
        (6, Some(false)),
        (7, None),
        (8, None),
        (10, Some(false)),
        (19, Some(false)),
        (20, None),
    ];
    for (code, retryable) in table {
        assert_eq!(exit::retryable(code), retryable, "exit code {code}");
    }
    for entry in catalog::entries() {
        let entry = entry.unwrap();
        assert_eq!(
            exit::retryable(entry.exit_code),
            Some(entry.retryable),
            "{}",
            entry.code
        );
    }
    let secret = project(&[(
        "project.toml",
        "name = \"sales\"\nnmae = 1\n\n[gold]\nlocation = \"s3://key:hunter2@bucket/gold\"\n",
    )]);
    let outcome = mergus(&secret, &["validate", "--json"]);
    assert_eq!(outcome.exit, 6, "the highest exit code among the errors");
    assert_eq!(outcome.of("run_finished")[0]["exit_code"], 6);
}

#[test]
fn mg_req_0264_actor_changes_nothing_but_audit() {
    let files = broken();
    let run = |args: &[&str], vars: &[(&str, &str)]| {
        let random = SimRandom::new(7);
        Setup {
            files: &files,
            cwd: ROOT,
            args,
            vars,
            random: &random,
            seed: 7,
        }
        .run()
    };
    let plain = run(&["validate", "--json"], &[]);
    let with_variable = run(
        &["validate", "--json"],
        &[("MERGUS_ACTOR", "{\"id\":\"eve\",\"kind\":\"person\"}")],
    );
    assert_eq!(
        plain, with_variable,
        "an actor in the environment is ignored"
    );
    let with_argument = run(&["validate", "--json", "--actor=eve"], &[]);
    let with_file = mergus(
        &project(&[("project.toml", "name = \"sales\"\nactor = \"eve\"\n")]),
        &["validate", "--json"],
    );
    for outcome in [plain, with_argument, with_file] {
        for event in outcome.events() {
            assert!(event.get("actor").is_none(), "{event}");
        }
    }
}
