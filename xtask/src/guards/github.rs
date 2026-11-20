//! Reads from the GitHub REST API with the token in `GITHUB_TOKEN` (MG-REQ-1730).
//!
//! `xtask` has no dependencies, so requests go through `curl`, which every GitHub runner has.
//! The token reaches `curl` on standard input, never on its command line.

use std::io::Write as _;
use std::process::{Command, Stdio};

use crate::failure::Failure;
use crate::json::Json;

/// Items asked for in each page of a list.
const PAGE_SIZE: usize = 100;

/// A source of GitHub REST API responses.
pub(crate) trait Api {
    /// Returns the JSON document at `path`, such as `/repos/o/r/pulls/1?page=2`.
    ///
    /// # Errors
    ///
    /// Fails if the request fails or the response is not JSON.
    fn get(&self, path: &str) -> Result<Json, Failure>;
}

/// The GitHub REST API at `GITHUB_API_URL` (by default `https://api.github.com`).
pub(crate) struct Rest {
    base: String,
    token: Option<String>,
}

impl Rest {
    /// Reads the API address and the token from the environment.
    pub(crate) fn from_env() -> Self {
        let base =
            std::env::var("GITHUB_API_URL").unwrap_or_else(|_| "https://api.github.com".to_owned());
        let token = std::env::var("GITHUB_TOKEN")
            .ok()
            .filter(|token| !token.is_empty());
        Self { base, token }
    }
}

impl Api for Rest {
    fn get(&self, path: &str) -> Result<Json, Failure> {
        let url = format!("{}{path}", self.base.trim_end_matches('/'));
        let mut child = Command::new("curl")
            .args(["--silent", "--show-error", "--fail", "--location"])
            .args(["--header", "Accept: application/vnd.github+json"])
            .args(["--header", "X-GitHub-Api-Version: 2022-11-28"])
            .args(["--header", "@-", &url])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| Failure::new(format!("cannot run curl: {error}")))?;
        if let (Some(stdin), Some(token)) = (child.stdin.as_mut(), &self.token) {
            writeln!(stdin, "Authorization: Bearer {token}")
                .map_err(|error| Failure::new(format!("cannot pass the token to curl: {error}")))?;
        }
        drop(child.stdin.take());
        let output = child
            .wait_with_output()
            .map_err(|error| Failure::new(format!("curl failed on {url}: {error}")))?;
        if !output.status.success() {
            return Err(Failure::new(format!("GET {url} failed: {}", output.status)));
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|_| Failure::new(format!("GET {url} returned non-UTF-8")))?;
        Json::parse(&text)
    }
}

/// A commit of a pull request.
pub(crate) struct Commit {
    /// Its hash.
    pub(crate) sha: String,
    /// Its full message.
    pub(crate) message: String,
    /// How many parents it has.
    pub(crate) parents: usize,
}

/// A file a pull request changes.
pub(crate) struct ChangedFile {
    /// Its path after the change, and before it when the change renames it.
    pub(crate) paths: Vec<String>,
    /// Its unified diff, when GitHub gives one.
    pub(crate) patch: Option<String>,
}

/// An issue: its body and the names of its labels.
pub(crate) struct Issue {
    /// Its body in Markdown.
    pub(crate) body: String,
    /// The names of its labels.
    pub(crate) labels: Vec<String>,
}

/// A pull request of the repository: its number and head branch.
pub(crate) struct PullRequest {
    /// Its number.
    pub(crate) number: u64,
    /// Its head branch.
    pub(crate) branch: String,
}

/// Returns every item of a paginated list.
///
/// # Errors
///
/// Fails if a request fails or a page is not a JSON array.
pub(crate) fn list(api: &dyn Api, path: &str) -> Result<Vec<Json>, Failure> {
    let separator = if path.contains('?') { '&' } else { '?' };
    let mut all = Vec::new();
    for page in 1.. {
        let url = format!("{path}{separator}per_page={PAGE_SIZE}&page={page}");
        let items = api
            .get(&url)?
            .into_items()
            .ok_or_else(|| Failure::new(format!("GET {url} did not return a list")))?;
        let last = items.len() < PAGE_SIZE;
        all.extend(items);
        if last {
            break;
        }
    }
    Ok(all)
}

/// Returns the commits of a pull request.
///
/// # Errors
///
/// Fails if the API fails.
pub(crate) fn commits(api: &dyn Api, repo: &str, number: u64) -> Result<Vec<Commit>, Failure> {
    let items = list(api, &format!("/repos/{repo}/pulls/{number}/commits"))?;
    Ok(items
        .iter()
        .map(|item| Commit {
            sha: text(item.get("sha")),
            message: text(item.get("commit").and_then(|commit| commit.get("message"))),
            parents: item
                .get("parents")
                .map_or(0, |parents| parents.items().len()),
        })
        .collect())
}

/// Returns the files a pull request changes.
///
/// # Errors
///
/// Fails if the API fails.
pub(crate) fn files(api: &dyn Api, repo: &str, number: u64) -> Result<Vec<ChangedFile>, Failure> {
    let items = list(api, &format!("/repos/{repo}/pulls/{number}/files"))?;
    Ok(items
        .iter()
        .map(|item| ChangedFile {
            paths: ["filename", "previous_filename"]
                .iter()
                .filter_map(|key| item.get(key).and_then(Json::as_str))
                .map(str::to_owned)
                .collect(),
            patch: item.get("patch").and_then(Json::as_str).map(str::to_owned),
        })
        .collect())
}

/// Returns an issue.
///
/// # Errors
///
/// Fails if the API fails.
pub(crate) fn issue(api: &dyn Api, repo: &str, number: u64) -> Result<Issue, Failure> {
    let item = api.get(&format!("/repos/{repo}/issues/{number}"))?;
    Ok(Issue {
        body: text(item.get("body")),
        labels: item
            .get("labels")
            .map(Json::items)
            .unwrap_or_default()
            .iter()
            .map(|label| text(label.get("name")))
            .collect(),
    })
}

/// Returns every pull request of the repository, open or closed.
///
/// # Errors
///
/// Fails if the API fails.
pub(crate) fn pull_requests(api: &dyn Api, repo: &str) -> Result<Vec<PullRequest>, Failure> {
    let items = list(api, &format!("/repos/{repo}/pulls?state=all"))?;
    Ok(items
        .iter()
        .filter_map(|item| {
            Some(PullRequest {
                number: item.get("number").and_then(Json::as_u64)?,
                branch: text(item.get("head").and_then(|head| head.get("ref"))),
            })
        })
        .collect())
}

/// The text of an optional JSON string, or an empty string.
fn text(value: Option<&Json>) -> String {
    value.and_then(Json::as_str).unwrap_or_default().to_owned()
}
