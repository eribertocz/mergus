//! MG-REQ-0501: `mergus validate` reports raw and gold behind the same credentials like the
//! other stage-1 problems.

use super::{mergus, project};

#[test]
fn mg_req_0501_validate_reports_shared_store() {
    let toml = "name = \"sales\"\n[raw]\nprofile = \"local\"\nlocation = \"lake\"\n\
                [gold]\nprofile = \"local\"\nlocation = \"lake/gold\"\n";
    let files = project(&[("project.toml", toml)]);
    let outcome = mergus(&files, &["validate", "--json"]);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    let diagnostics = outcome.of("diagnostic");
    assert_eq!(diagnostics.len(), 1, "{outcome:?}");
    let error = &diagnostics[0];
    assert_eq!(error["code"], "MG-0501");
    assert_eq!(error["severity"], "error");
    assert_eq!(error["location"]["file"], "project.toml");
    assert_eq!(error["location"]["line"], 7);
    let message = error["message"].as_str().unwrap();
    assert!(message.contains("location `lake`"), "{message}");
    assert_eq!(outcome.of("run_finished")[0]["status"], "failed");

    let apart = "name = \"sales\"\n[raw]\nprofile = \"local\"\nlocation = \"lake/raw\"\n\
                 [gold]\nprofile = \"local\"\nlocation = \"lake/gold\"\n";
    let files = project(&[("project.toml", apart)]);
    let outcome = mergus(&files, &["validate", "--json"]);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
}
