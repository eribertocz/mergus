//! The error catalog of `errors/`, compiled into the binary: the exit code, whether it is
//! retryable, the hint and the cause of every code (MG-REQ-0008, MG-REQ-1334, MG-REQ-1346).

/// The text of every catalog file, by code.
const FILES: [(&str, &str); 63] = [
    ("MG-0101", include_str!("../errors/MG-0101.md")),
    ("MG-0102", include_str!("../errors/MG-0102.md")),
    ("MG-0301", include_str!("../errors/MG-0301.md")),
    ("MG-0302", include_str!("../errors/MG-0302.md")),
    ("MG-0303", include_str!("../errors/MG-0303.md")),
    ("MG-0304", include_str!("../errors/MG-0304.md")),
    ("MG-0305", include_str!("../errors/MG-0305.md")),
    ("MG-0306", include_str!("../errors/MG-0306.md")),
    ("MG-0307", include_str!("../errors/MG-0307.md")),
    ("MG-0308", include_str!("../errors/MG-0308.md")),
    ("MG-0309", include_str!("../errors/MG-0309.md")),
    ("MG-0310", include_str!("../errors/MG-0310.md")),
    ("MG-0311", include_str!("../errors/MG-0311.md")),
    ("MG-0312", include_str!("../errors/MG-0312.md")),
    ("MG-0313", include_str!("../errors/MG-0313.md")),
    ("MG-0314", include_str!("../errors/MG-0314.md")),
    ("MG-0315", include_str!("../errors/MG-0315.md")),
    ("MG-0401", include_str!("../errors/MG-0401.md")),
    ("MG-0402", include_str!("../errors/MG-0402.md")),
    ("MG-0403", include_str!("../errors/MG-0403.md")),
    ("MG-0404", include_str!("../errors/MG-0404.md")),
    ("MG-0405", include_str!("../errors/MG-0405.md")),
    ("MG-0406", include_str!("../errors/MG-0406.md")),
    ("MG-0407", include_str!("../errors/MG-0407.md")),
    ("MG-0408", include_str!("../errors/MG-0408.md")),
    ("MG-0409", include_str!("../errors/MG-0409.md")),
    ("MG-0410", include_str!("../errors/MG-0410.md")),
    ("MG-0411", include_str!("../errors/MG-0411.md")),
    ("MG-0412", include_str!("../errors/MG-0412.md")),
    ("MG-0413", include_str!("../errors/MG-0413.md")),
    ("MG-0501", include_str!("../errors/MG-0501.md")),
    ("MG-0502", include_str!("../errors/MG-0502.md")),
    ("MG-0503", include_str!("../errors/MG-0503.md")),
    ("MG-0504", include_str!("../errors/MG-0504.md")),
    ("MG-0505", include_str!("../errors/MG-0505.md")),
    ("MG-0506", include_str!("../errors/MG-0506.md")),
    ("MG-0507", include_str!("../errors/MG-0507.md")),
    ("MG-0601", include_str!("../errors/MG-0601.md")),
    ("MG-0602", include_str!("../errors/MG-0602.md")),
    ("MG-0603", include_str!("../errors/MG-0603.md")),
    ("MG-0604", include_str!("../errors/MG-0604.md")),
    ("MG-0605", include_str!("../errors/MG-0605.md")),
    ("MG-0606", include_str!("../errors/MG-0606.md")),
    ("MG-0607", include_str!("../errors/MG-0607.md")),
    ("MG-0701", include_str!("../errors/MG-0701.md")),
    ("MG-0702", include_str!("../errors/MG-0702.md")),
    ("MG-0703", include_str!("../errors/MG-0703.md")),
    ("MG-1001", include_str!("../errors/MG-1001.md")),
    ("MG-1002", include_str!("../errors/MG-1002.md")),
    ("MG-1003", include_str!("../errors/MG-1003.md")),
    ("MG-1301", include_str!("../errors/MG-1301.md")),
    ("MG-1302", include_str!("../errors/MG-1302.md")),
    ("MG-1303", include_str!("../errors/MG-1303.md")),
    ("MG-1304", include_str!("../errors/MG-1304.md")),
    ("MG-1305", include_str!("../errors/MG-1305.md")),
    ("MG-1306", include_str!("../errors/MG-1306.md")),
    ("MG-1307", include_str!("../errors/MG-1307.md")),
    ("MG-1308", include_str!("../errors/MG-1308.md")),
    ("MG-1309", include_str!("../errors/MG-1309.md")),
    ("MG-1310", include_str!("../errors/MG-1310.md")),
    ("MG-1311", include_str!("../errors/MG-1311.md")),
    ("MG-1312", include_str!("../errors/MG-1312.md")),
    ("MG-1313", include_str!("../errors/MG-1313.md")),
];

/// What the catalog says about one code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    /// The `MG-####` code.
    pub(crate) code: &'static str,
    /// The exit code of a command that ends with this error.
    pub(crate) exit_code: u8,
    /// Whether running the command again may succeed.
    pub(crate) retryable: bool,
    /// The first sentence of the `Fix` section: what to do next.
    pub(crate) hint: String,
    /// The first paragraph of the `Cause` section, on one line: why the error matters
    /// (MG-REQ-1346.3).
    pub(crate) cause: String,
}

/// Returns the catalog entry of `code`, or `None` when it has no well-formed file.
pub(crate) fn entry(code: &str) -> Option<Entry> {
    let (code, text) = FILES.iter().find(|(known, _)| *known == code)?;
    parse(code, text)
}

/// Returns the text of the catalog file of `code`, or `None` when the code has none
/// (MG-REQ-1352.3).
pub(crate) fn file(code: &str) -> Option<&'static str> {
    FILES
        .iter()
        .find(|(known, _)| *known == code)
        .map(|(_, text)| *text)
}

/// Returns every code of the catalog, sorted.
pub(crate) fn codes() -> impl Iterator<Item = &'static str> {
    FILES.iter().map(|(code, _)| *code)
}

/// Returns every entry of the catalog, `None` for a file that is not well formed.
#[cfg(test)]
pub(crate) fn entries() -> Vec<Option<Entry>> {
    FILES.iter().map(|(code, text)| parse(code, text)).collect()
}

fn parse(code: &'static str, text: &str) -> Option<Entry> {
    let value = |prefix: &str| text.lines().find_map(|line| line.strip_prefix(prefix));
    let exit_code = value("- Exit code: ")?.trim().parse().ok()?;
    let retryable = match value("- Retryable: ")?.trim() {
        "yes" => true,
        "no" => false,
        _ => return None,
    };
    let fix = paragraph(text, "## Fix");
    let cause = paragraph(text, "## Cause");
    let hint = match fix.find(". ") {
        Some(end) => &fix[..=end],
        None => fix.as_str(),
    };
    (!hint.is_empty() && !cause.is_empty()).then(|| Entry {
        code,
        exit_code,
        retryable,
        hint: hint.to_owned(),
        cause,
    })
}

/// The first paragraph under `heading`, its lines joined by a space.
fn paragraph(text: &str, heading: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .skip_while(|line| line.trim_end() != heading)
        .skip(1)
        .take_while(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(str::trim)
        .collect();
    lines.join(" ")
}

#[cfg(test)]
mod tests {
    use mergus_ports::FileSystem;
    use mergus_ports::real::RealFileSystem;

    use super::{FILES, entries, entry};

    #[test]
    fn mg_req_0721_missing_secret_is_a_security_failure() {
        // MG-REQ-0262: a missing secret ends with the exit code of security failures, 6.
        let missing = entry("MG-0701").unwrap();
        assert_eq!(missing.exit_code, 6);
        assert!(!missing.retryable);
    }

    #[test]
    fn every_catalog_file_is_compiled_in_and_well_formed() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("errors");
        let on_disk: Vec<String> = RealFileSystem
            .list_dir(&dir)
            .unwrap()
            .iter()
            .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        let compiled: Vec<&str> = FILES.iter().map(|(code, _)| *code).collect();
        assert_eq!(on_disk, compiled);
        for entry in entries() {
            let entry = entry.unwrap();
            assert!(entry.hint.ends_with('.'), "{}: {}", entry.code, entry.hint);
        }
    }
}
