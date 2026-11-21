//! Tests of `cargo xtask owner-queue` against the fixtures under
//! `xtask/fixtures/owner-queue/<case>/api.json` (MG-REQ-1713). They never call the network.

use std::path::Path;

use super::owner_queue::{items, repository_from_remote};
use super::tests::Fixture;
use crate::json::Json;
use crate::workspace;

/// The lines of the queue of a fixture.
fn queue(case: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/owner-queue")
        .join(case)
        .join("api.json");
    let api = Fixture(Json::parse(&workspace::read(&path).unwrap()).unwrap());
    items(&api, "owner/repo").unwrap()
}

#[test]
fn mg_req_1713_owner_queue_lists_ready_items() {
    assert_eq!(
        queue("ready-items"),
        [
            "merge #12 Ready change https://github.com/owner/repo/pull/12",
            "merge #14 Spec change https://github.com/owner/repo/pull/14",
            "needs-owner #20 Owner decision https://github.com/owner/repo/issues/20",
            "tag #31 Release 0.2.0 https://github.com/owner/repo/pull/31",
        ]
    );
}

#[test]
fn mg_req_1713_4_an_empty_queue_has_no_line() {
    assert_eq!(queue("empty"), Vec::<String>::new());
}

#[test]
fn mg_req_1713_the_repository_is_read_from_the_remote() {
    for url in [
        "https://github.com/owner/repo.git",
        "https://github.com/owner/repo",
        "git@github.com:owner/repo.git",
        "http://proxy@127.0.0.1:1234/git/owner/repo\n",
    ] {
        assert_eq!(
            repository_from_remote(url).as_deref(),
            Some("owner/repo"),
            "{url}"
        );
    }
    assert_eq!(repository_from_remote("repo"), None);
}
