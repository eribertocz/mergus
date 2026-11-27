//! The libpq parameters of a protected PostgreSQL session (MG-REQ-0429.2 and MG-REQ-0429.3),
//! added in memory to a connection value before it goes into the temporary secret.

use crate::PortError;

/// The parameters Mergus adds, as keyword and value.
const PARAMETERS: [(&str, &str); 2] = [
    ("application_name", "mergus"),
    ("options", "-c lock_timeout=5000"),
];

/// Returns `connection`, the connection value of the source attached as `source`, with
/// `application_name=mergus` and `options=-c lock_timeout=5000` added in its own form: as
/// percent-encoded query parameters of a `postgresql://` or `postgres://` URI, or as
/// keywords of a keyword/value string (MG-REQ-0429.2).
///
/// # Errors
///
/// Fails with [`PortError::SessionParameterSet`] when the value already sets either
/// parameter (MG-REQ-0429.3). The error never holds the value.
pub(crate) fn protected_connection(source: &str, connection: &str) -> Result<String, PortError> {
    let uri = ["postgresql://", "postgres://"]
        .iter()
        .any(|scheme| connection.starts_with(scheme));
    let keywords = if uri {
        uri_keywords(connection)
    } else {
        keyword_value_keywords(connection)
    };
    if let Some((parameter, _)) = PARAMETERS
        .iter()
        .find(|(parameter, _)| keywords.iter().any(|keyword| keyword == parameter))
    {
        return Err(PortError::SessionParameterSet {
            source_name: source.to_owned(),
            parameter: (*parameter).to_owned(),
        });
    }
    let mut protected = connection.to_owned();
    if uri {
        for (keyword, value) in PARAMETERS {
            if !protected.contains('?') {
                protected.push('?');
            } else if !protected.ends_with(['?', '&']) {
                protected.push('&');
            }
            protected.push_str(keyword);
            protected.push('=');
            protected.push_str(&percent_encode(value));
        }
    } else {
        let trimmed = protected.trim_end().len();
        protected.truncate(trimmed);
        for (keyword, value) in PARAMETERS {
            if !protected.is_empty() {
                protected.push(' ');
            }
            protected.push_str(keyword);
            protected.push('=');
            protected.push_str(&quote(value));
        }
    }
    Ok(protected)
}

/// Returns the percent-decoded keywords of the query parameters of the URI `uri`.
fn uri_keywords(uri: &str) -> Vec<String> {
    let Some((_, query)) = uri.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .map(|pair| percent_decode(pair.split_once('=').map_or(pair, |(keyword, _)| keyword)))
        .collect()
}

/// Returns the keywords of the keyword/value string `text`: each `keyword = value` pair,
/// with spaces allowed around `=`, and a value that is single-quoted, with `\'` and `\\`
/// escaped, or ends at the next space.
fn keyword_value_keywords(text: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    let mut chars = text.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let mut keyword = String::new();
        while let Some(c) = chars.next_if(|c| *c != '=' && !c.is_whitespace()) {
            keyword.push(c);
        }
        if keyword.is_empty() && chars.peek().is_none() {
            return keywords;
        }
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        if chars.next_if_eq(&'=').is_none() {
            // Not a keyword/value string libpq accepts; libpq reports it at the attach.
            return keywords;
        }
        keywords.push(keyword);
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let quoted = chars.next_if_eq(&'\'').is_some();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '\'' if quoted => break,
                c if c.is_whitespace() && !quoted => break,
                _ => {}
            }
        }
    }
}

/// Returns `value` as a keyword/value string writes it: as it is when it has no space,
/// `'` or `\`, otherwise single-quoted with `'` and `\` escaped.
fn quote(value: &str) -> String {
    let plain = !value.is_empty()
        && !value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '\'' | '\\'));
    if plain {
        return value.to_owned();
    }
    let mut quoted = String::from('\'');
    for c in value.chars() {
        if matches!(c, '\'' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('\'');
    quoted
}

/// The upper-case hexadecimal digits.
const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Returns `value` with every byte other than an unreserved URI character percent-encoded.
fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

/// Returns `text` with its `%XX` escapes decoded; an escape that is not valid is kept.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        let escaped = bytes
            .get(index + 1..index + 3)
            .filter(|_| byte == b'%')
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        if let Some(escaped) = escaped {
            decoded.push(escaped);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
