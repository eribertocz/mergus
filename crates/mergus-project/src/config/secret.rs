//! The literal secrets of MG-REQ-0363, rejected by MG-REQ-0320.

/// The key suffixes of MG-REQ-0363.3, in lowercase.
const SECRET_KEYS: [&str; 7] = [
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "private_key",
];

/// Returns whether the string `text`, held by `key`, is a literal secret (MG-REQ-0363). The
/// key of an array item is the key of its array; `None` when the value has no key.
pub(crate) fn is_literal_secret(key: Option<&str>, text: &str) -> bool {
    if text.starts_with("secret:") {
        return false;
    }
    has_url_password(text) || has_private_key(text) || key.is_some_and(|key| secret_key(key, text))
}

/// MG-REQ-0363.1: `<scheme>://<user>:<password>@`, with a password that is not empty.
fn has_url_password(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once("://") else {
        return false;
    };
    let mut scheme_chars = scheme.chars();
    let valid_scheme = scheme_chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && scheme_chars.all(|char| char.is_ascii_alphanumeric() || "+-.".contains(char));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let user_info = authority.rsplit_once('@').map(|(user_info, _)| user_info);
    let password = user_info.and_then(|user_info| user_info.split_once(':'));
    valid_scheme && password.is_some_and(|(_, password)| !password.is_empty())
}

/// MG-REQ-0363.2: `-----BEGIN ` followed, on the same line, by `PRIVATE KEY-----`.
fn has_private_key(text: &str) -> bool {
    text.lines().any(|line| {
        line.split_once("-----BEGIN ")
            .is_some_and(|(_, rest)| rest.contains("PRIVATE KEY-----"))
    })
}

/// MG-REQ-0363.3: a key that ends with a secret word, without case, and a value.
fn secret_key(key: &str, text: &str) -> bool {
    let key = key.to_ascii_lowercase();
    !text.is_empty() && SECRET_KEYS.iter().any(|suffix| key.ends_with(suffix))
}
