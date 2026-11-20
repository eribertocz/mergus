//! Sections of a Markdown body, as the format guards read them (MG-REQ-1735, MG-REQ-1736).

/// The opening and the closing of an HTML comment.
const COMMENT: (&str, &str) = ("<!--", "-->");

/// Returns whether a line is an ATX heading: one to six `#`, then a space or nothing.
pub(crate) fn is_heading(line: &str) -> bool {
    let line = line.trim();
    let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
    (1..=6).contains(&hashes)
        && line
            .get(hashes..)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Returns the heading lines of a text, trimmed, in order.
pub(crate) fn headings(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| is_heading(line))
        .map(str::trim)
        .collect()
}

/// Returns the text under the first heading written exactly as `heading`, up to the next
/// heading, or `None` when the text has no such heading.
pub(crate) fn section(text: &str, heading: &str) -> Option<String> {
    let mut lines = text
        .lines()
        .skip_while(|line| !(is_heading(line) && line.trim() == heading));
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|line| !is_heading(line)).collect();
    Some(body.join("\n"))
}

/// Returns whether a section has text: something other than blanks and HTML comments.
pub(crate) fn has_text(section: &str) -> bool {
    let mut rest = section;
    loop {
        let Some((before, after)) = rest.split_once(COMMENT.0) else {
            return !rest.trim().is_empty();
        };
        if !before.trim().is_empty() {
            return true;
        }
        let Some((_, after)) = after.split_once(COMMENT.1) else {
            return false;
        };
        rest = after;
    }
}

/// Returns the rule IDs a text names, in order: `MG-REQ-` followed by digits and any
/// `.<digits>` item numbers.
pub(crate) fn rule_ids(text: &str) -> Vec<&str> {
    const PREFIX: &str = "MG-REQ-";
    let mut ids = Vec::new();
    for (at, _) in text.match_indices(PREFIX) {
        let rest = text.get(at + PREFIX.len()..).unwrap_or_default();
        let mut end = digits(rest);
        if end == 0 {
            continue;
        }
        while rest.get(end..).is_some_and(|tail| tail.starts_with('.')) {
            let more = digits(rest.get(end + 1..).unwrap_or_default());
            if more == 0 {
                break;
            }
            end += 1 + more;
        }
        if let Some(id) = text.get(at..at + PREFIX.len() + end) {
            ids.push(id);
        }
    }
    ids
}

/// The length of the run of ASCII digits at the start of `text`.
fn digits(text: &str) -> usize {
    text.bytes().take_while(u8::is_ascii_digit).count()
}

#[cfg(test)]
mod tests {
    use super::{has_text, headings, rule_ids, section};

    #[test]
    fn mg_req_1735_sections_end_at_the_next_heading() {
        let text = "intro\n## Rules\n- MG-REQ-1\n\n### Sub\nx\n## Crate\r\nnone\r\n";
        assert_eq!(headings(text), ["## Rules", "### Sub", "## Crate"]);
        assert_eq!(section(text, "## Rules").as_deref(), Some("- MG-REQ-1\n"));
        assert_eq!(section(text, "## Crate").as_deref(), Some("none"));
        assert_eq!(section(text, "## Files"), None);
        assert!(headings("#no space\n####### seven\n").is_empty());
    }

    #[test]
    fn mg_req_1735_comments_and_blanks_are_not_text() {
        assert!(!has_text(""));
        assert!(!has_text("  \n<!-- a\nb -->\n<!-- c -->  \n"));
        assert!(!has_text("<!-- unclosed"));
        assert!(has_text("<!-- a --> none"));
        assert!(has_text("x <!-- a -->"));
    }

    #[test]
    fn mg_req_1736_rule_ids_are_read_with_their_items() {
        assert_eq!(
            rule_ids("MG-REQ-1734, MG-REQ-1738.2. MG-REQ- MG-REQ-12.x `MG-REQ-0001`"),
            ["MG-REQ-1734", "MG-REQ-1738.2", "MG-REQ-12", "MG-REQ-0001"]
        );
    }
}
