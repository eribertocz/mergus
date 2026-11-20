//! Branch names of MG-REQ-1707, as the `scope` guard reads them (MG-REQ-1731.1).

/// Returns whether `branch` is named `issue-<n>-<words>`, `spec/<words>` or
/// `release/<major>.<minor>.<patch>` (MG-REQ-1731.1).
pub(crate) fn is_valid(branch: &str) -> bool {
    if let Some(rest) = branch.strip_prefix("issue-") {
        return rest
            .split_once('-')
            .is_some_and(|(number, name)| is_number(number) && is_words(name));
    }
    if let Some(name) = branch.strip_prefix("spec/") {
        return is_words(name);
    }
    if let Some(version) = branch.strip_prefix("release/") {
        let parts: Vec<&str> = version.split('.').collect();
        return parts.len() == 3 && parts.iter().all(|part| is_version_part(part));
    }
    false
}

/// Returns whether a pull request with this head branch is an implementation pull request:
/// its branch starts with `issue-`.
pub(crate) fn is_implementation(branch: &str) -> bool {
    branch.starts_with("issue-")
}

/// Returns the issue of an implementation branch: the positive number after `issue-`.
pub(crate) fn issue(branch: &str) -> Option<u64> {
    let rest = branch.strip_prefix("issue-")?;
    let number = rest.split('-').next().unwrap_or_default();
    if is_number(number) {
        number.parse().ok()
    } else {
        None
    }
}

/// A positive integer without leading zero.
fn is_number(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_digit()) && !text.is_empty() && !text.starts_with('0')
}

/// One part of a release version: `0`, or a positive integer without leading zero.
fn is_version_part(text: &str) -> bool {
    text == "0" || is_number(text)
}

/// Lowercase letters and digits in words joined by single hyphens.
fn is_words(text: &str) -> bool {
    text.split('-').all(|word| {
        !word.is_empty()
            && word
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::{is_valid, issue};

    #[test]
    fn mg_req_1731_branch_names_follow_the_three_patterns() {
        for name in [
            "issue-9-guards-one",
            "issue-12-a",
            "spec/report-on-github",
            "spec/v2",
            "release/0.1.0",
            "release/10.20.30",
        ] {
            assert!(is_valid(name), "{name} should be valid");
        }
        for name in [
            "main",
            "issue-9",
            "issue-0-zero",
            "issue-09-leading",
            "issue-9-Upper",
            "issue-9-double--hyphen",
            "issue-9-trailing-",
            "issue-x-words",
            "spec/",
            "spec/under_score",
            "spec/a/b",
            "release/0.1",
            "release/0.1.0.0",
            "release/01.0.0",
            "release/v0.1.0",
            "feature/x",
        ] {
            assert!(!is_valid(name), "{name} should be invalid");
        }
    }

    #[test]
    fn mg_req_1731_the_issue_is_the_number_in_the_branch() {
        assert_eq!(issue("issue-9-guards-one"), Some(9));
        assert_eq!(issue("issue-123"), Some(123));
        assert_eq!(issue("issue-09-x"), None);
        assert_eq!(issue("spec/x"), None);
    }
}
