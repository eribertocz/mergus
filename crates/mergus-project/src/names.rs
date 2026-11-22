//! Name rules: valid names (MG-REQ-0305), suggestions (MG-REQ-0360) and the closest known
//! name (MG-REQ-0359).

/// The longest valid name, in bytes (MG-REQ-0305).
const MAX_NAME_LENGTH: usize = 63;

/// Returns whether `name` matches `^[a-z][a-z0-9_]{0,62}$` (MG-REQ-0305).
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && name.len() <= MAX_NAME_LENGTH
        && bytes.all(is_name_byte)
}

/// Returns whether `byte` is one of `a-z`, `0-9` and `_`.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
}

/// Returns the valid name suggested for `name` (MG-REQ-0360): ASCII letters lowercased,
/// every run of other characters replaced by one `_`, leading and trailing `_` removed, `n_`
/// prepended when the result is empty or starts with a digit, then cut to 63 characters.
#[must_use]
pub fn suggest_valid_name(name: &str) -> String {
    let mut replaced = String::with_capacity(name.len());
    let mut in_run = false;
    for character in name.chars().map(|character| character.to_ascii_lowercase()) {
        if character.is_ascii() && is_name_byte(character as u8) {
            replaced.push(character);
            in_run = false;
        } else if !in_run {
            replaced.push('_');
            in_run = true;
        }
    }
    let trimmed = replaced.trim_matches('_');
    let mut suggestion = if trimmed.is_empty() || trimmed.starts_with(|c: char| c.is_ascii_digit())
    {
        format!("n_{trimmed}")
    } else {
        trimmed.to_owned()
    };
    // Every character is ASCII here, so a byte length is a character count.
    suggestion.truncate(MAX_NAME_LENGTH);
    suggestion
}

/// Returns the candidate closest to `given` by Levenshtein distance counted in Unicode scalar
/// values, ties broken by byte order, or `None` when that distance is more than
/// `max(2, n / 3)`, `n` being the number of scalar values of `given` (MG-REQ-0359).
#[must_use]
pub fn closest_name<'a, I>(given: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let given: Vec<char> = given.chars().collect();
    let limit = (given.len() / 3).max(2);
    candidates
        .into_iter()
        .map(|candidate| (levenshtein(&given, candidate), candidate))
        .min()
        .filter(|(distance, _)| *distance <= limit)
        .map(|(_, candidate)| candidate)
}

/// Returns the Levenshtein distance between `given` and `candidate`, in scalar values.
fn levenshtein(given: &[char], candidate: &str) -> usize {
    let mut previous: Vec<usize> = (0..=given.len()).collect();
    for (row, other) in candidate.chars().enumerate() {
        let mut current = Vec::with_capacity(previous.len());
        current.push(row + 1);
        for (column, character) in given.iter().enumerate() {
            let substitution = previous[column] + usize::from(*character != other);
            let deletion = previous[column + 1] + 1;
            let insertion = current[column] + 1;
            current.push(substitution.min(deletion).min(insertion));
        }
        previous = current;
    }
    previous[given.len()]
}
