//! Commit messages: subject and trailers (MG-REQ-1732, MG-REQ-1733).

/// The types of a Conventional Commit subject (MG-REQ-1732.1).
const TYPES: [&str; 11] = [
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

/// The prefix of a valid `Claude-Session:` value (MG-REQ-1732.3).
const SESSION_PREFIX: &str = "https://claude.ai/code/session_";

/// Returns the first line of a commit message.
pub(crate) fn subject(message: &str) -> &str {
    message.lines().next().unwrap_or_default()
}

/// Returns whether a subject matches `<type>(<scope>)!: <text>`, `(<scope>)` and `!` being
/// optional (MG-REQ-1732.1).
pub(crate) fn is_conventional(subject: &str) -> bool {
    let Some((head, text)) = subject.split_once(": ") else {
        return false;
    };
    if text.trim().is_empty() {
        return false;
    }
    let head = head.strip_suffix('!').unwrap_or(head);
    let (kind, scope) = match head.split_once('(') {
        Some((kind, rest)) => match rest.strip_suffix(')') {
            Some(scope) => (kind, Some(scope)),
            None => return false,
        },
        None => (head, None),
    };
    TYPES.contains(&kind)
        && scope.is_none_or(|scope| !scope.is_empty() && !scope.contains(['(', ')']))
}

/// Returns the trailers of a message: the `Key: value` lines of its last paragraph, when the
/// message has a paragraph after the subject. Keys keep their case.
pub(crate) fn trailers(message: &str) -> Vec<(&str, &str)> {
    let lines: Vec<&str> = message.lines().collect();
    let Some(blank) = lines.iter().rposition(|line| line.trim().is_empty()) else {
        return Vec::new();
    };
    lines
        .get(blank + 1..)
        .unwrap_or_default()
        .iter()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| !key.is_empty() && !key.contains(char::is_whitespace))
        .map(|(key, value)| (key, value.trim()))
        .collect()
}

/// Returns the values of the trailers named `key`, ignoring the case of the key.
pub(crate) fn values<'a>(message: &'a str, key: &str) -> Vec<&'a str> {
    trailers(message)
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value)
        .collect()
}

/// Returns whether a `Claude-Session:` value is `https://claude.ai/code/session_` followed by
/// letters and digits (MG-REQ-1732.3).
pub(crate) fn is_session(value: &str) -> bool {
    value
        .strip_prefix(SESSION_PREFIX)
        .is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::{is_conventional, is_session, values};

    #[test]
    fn mg_req_1732_subjects_follow_conventional_commits() {
        for subject in [
            "feat: add a guard",
            "fix(xtask): read the event (#9)",
            "refactor!: drop the old reader",
            "build(deps)!: bump",
        ] {
            assert!(is_conventional(subject), "{subject} should pass");
        }
        for subject in [
            "add a guard",
            "feature: add",
            "feat:add",
            "feat: ",
            "feat(): empty scope",
            "feat(a(b)): nested",
            "feat(xtask: unclosed",
            "Feat: upper",
        ] {
            assert!(!is_conventional(subject), "{subject} should fail");
        }
    }

    #[test]
    fn mg_req_1732_trailers_are_read_from_the_last_paragraph() {
        let message = "feat: x\n\nBody: not a trailer here\n\nco-authored-by: A <a@b>\n\
                       Claude-Session: https://claude.ai/code/session_01Ab\n";
        assert_eq!(values(message, "Co-Authored-By"), ["A <a@b>"]);
        assert_eq!(values(message, "Body"), Vec::<&str>::new());
        assert!(values("feat: x", "feat").is_empty());
    }

    #[test]
    fn mg_req_1732_session_values_are_checked() {
        assert!(is_session("https://claude.ai/code/session_01BBcaD3"));
        assert!(!is_session("https://claude.ai/code/session_"));
        assert!(!is_session("https://claude.ai/code/session_01-x"));
        assert!(!is_session("http://claude.ai/code/session_01"));
    }
}
