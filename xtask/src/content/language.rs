//! The `language` check: lines with an alphabetic character outside ASCII (MG-REQ-0177).

use super::{File, Finding};

/// Reports every line of a text file that contains an alphabetic character outside ASCII.
pub(super) fn findings(file: &File) -> Vec<Finding> {
    let Some(text) = &file.text else {
        return Vec::new();
    };
    text.lines()
        .zip(1..)
        .filter_map(|(line, number)| {
            let letter = line.chars().find(|c| c.is_alphabetic() && !c.is_ascii())?;
            let message = format!("non-ASCII letter `{letter}` (MG-REQ-0177)");
            Some(file.finding(number, "language", message))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::LANGUAGE;
    use super::super::tests::fixture_findings;

    #[test]
    fn mg_req_0177_lines_with_non_ascii_letters_are_reported() {
        assert_eq!(
            fixture_findings(LANGUAGE, "letters"),
            [
                "notes.md:2: language: non-ASCII letter `\u{e9}` (MG-REQ-0177)",
                "src/a.rs:3: language: non-ASCII letter `\u{3b1}` (MG-REQ-0177)",
            ]
        );
    }

    #[test]
    fn mg_req_0177_symbols_and_binary_files_are_not_reported() {
        assert_eq!(fixture_findings(LANGUAGE, "clean"), Vec::<String>::new());
    }
}
