//! Tests of the `release-trigger` check on workflows written inline (MG-REQ-1715).

use super::{check, judge};
use crate::workspace;

/// A finding of a workflow that grants `grants` without being a release workflow.
fn finding(grants: &str) -> Result<Option<String>, String> {
    Ok(Some(format!(
        "MG-REQ-1715: grants {grants} and is triggered by something other than a push of \
         tags matching `v*`"
    )))
}

#[test]
fn mg_req_1715_a_release_workflow_may_write() {
    for workflow in [
        "on:\n  push:\n    tags: ['v*']\npermissions:\n  contents: write\n",
        "on:\n  push:\n    tags:\n      - \"v*\"  # releases\n      - '!v0.0.*'\n\
         jobs:\n  publish:\n    permissions: { id-token: write, packages: write }\n    \
         steps:\n      - run: |\n          echo on: push\n",
        "\"on\": { push: { tags: [v*.*.*] } }\npermissions: write-all\n",
    ] {
        assert_eq!(judge(workflow), Ok(None), "{workflow}");
    }
}

#[test]
fn mg_req_1715_a_workflow_that_only_reads_passes() {
    for workflow in [
        "on: [pull_request, push]\npermissions:\n  contents: read\n  actions: write\n",
        "on: pull_request\npermissions: read-all\njobs:\n  a:\n    permissions:\n      \
         contents: read\n    steps:\n      - run: |\n          contents: write\n",
        "on: push\njobs:\n  a:\n    runs-on: ubuntu-latest\n",
    ] {
        assert_eq!(judge(workflow), Ok(None), "{workflow}");
    }
}

#[test]
fn mg_req_1715_a_writing_workflow_with_another_trigger_fails() {
    let cases = [
        (
            "on: push\npermissions:\n  contents: write\n",
            "`contents: write`",
        ),
        (
            "on:\n  push:\n    tags: [v*]\n  workflow_dispatch:\npermissions:\n  \
             packages: write\n",
            "`packages: write`",
        ),
        (
            "on:\n  push:\n    branches: [main]\n    tags: [v*]\njobs:\n  a:\n    \
             permissions:\n      id-token: write\n",
            "`id-token: write`",
        ),
        (
            "on:\n  push:\n    tags: ['*']\npermissions: write-all\n",
            "`contents: write`, `packages: write`, `id-token: write`",
        ),
        (
            "on:\n  push:\n    tags-ignore: [x]\npermissions: {contents: write}\n",
            "`contents: write`",
        ),
        (
            "on:\n  pull_request_target:\njobs:\n  a:\n    permissions:\n      \
             contents: write # needed\n",
            "`contents: write`",
        ),
    ];
    for (workflow, grants) in cases {
        assert_eq!(judge(workflow), finding(grants), "{workflow}");
    }
}

#[test]
fn mg_req_1715_a_workflow_that_does_not_parse_fails() {
    assert!(judge("on:\n  push:\n      tags: [v*]\n    bad: x\n").is_err());
    assert!(judge("on: [push\n").is_err());
}

#[test]
fn mg_req_1715_the_workflows_of_this_repository_pass() {
    assert!(check(&workspace::root().unwrap()).is_ok());
}
