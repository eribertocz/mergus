//! MG-REQ-0170: `unsafe` only in the crates that bind foreign code.

use std::path::{Path, PathBuf};

use crate::failure::Failure;
use crate::workspace::{self, Member};

/// The crates that bind foreign code (DuckDB, Polars, Python) and may use `unsafe`.
const FFI_CRATES: [&str; 3] = ["mergus-engine-duckdb", "mergus-engine-polars", "mergus-py"];

/// The attribute every other crate root must carry.
const FORBID: &str = "#![forbid(unsafe_code)]";

/// Checks that every crate root outside the FFI crates forbids `unsafe`, and that every
/// `unsafe` block of an FFI crate is preceded by a `// SAFETY:` comment. Files are shown
/// relative to the workspace `root`.
///
/// # Errors
///
/// Fails with the list of crate roots and blocks that break the rule.
pub(crate) fn check(root: &Path, members: &[Member]) -> Result<(), Failure> {
    let mut problems = Vec::new();
    for member in members {
        if FFI_CRATES.contains(&member.name.as_str()) {
            for file in workspace::rust_files(&member.dir)? {
                let source = workspace::read(&file)?;
                for line in unjustified_unsafe_blocks(&source) {
                    problems.push(format!(
                        "{}:{line}: `unsafe` block without a `// SAFETY:` comment",
                        workspace::show(root, &file)
                    ));
                }
            }
        } else {
            for file in crate_roots(&member.dir)? {
                if !forbids_unsafe(&workspace::read(&file)?) {
                    let shown = workspace::show(root, &file);
                    problems.push(format!("{shown}: missing `{FORBID}`"));
                }
            }
        }
    }
    Failure::from_problems("unsafe", &problems)
}

/// Lists the crate roots of a member: `src/lib.rs`, `src/main.rs` and `src/bin/*.rs`.
fn crate_roots(dir: &Path) -> Result<Vec<PathBuf>, Failure> {
    let mut roots: Vec<PathBuf> = ["src/lib.rs", "src/main.rs"]
        .iter()
        .map(|file| dir.join(file))
        .filter(|path| path.is_file())
        .collect();
    let bin = dir.join("src").join("bin");
    if bin.is_dir() {
        for path in workspace::sorted_entries(&bin)? {
            if path.extension().is_some_and(|extension| extension == "rs") {
                roots.push(path);
            } else if path.join("main.rs").is_file() {
                roots.push(path.join("main.rs"));
            }
        }
    }
    Ok(roots)
}

/// Returns whether a crate root carries `#![forbid(unsafe_code)]` on a line of its own.
fn forbids_unsafe(source: &str) -> bool {
    source.lines().any(|line| line.trim() == FORBID)
}

/// Returns the 1-based line numbers of `unsafe` blocks not preceded by a `// SAFETY:`
/// comment. The comment may span several lines; it must end on the line before the block.
fn unjustified_unsafe_blocks(source: &str) -> Vec<usize> {
    let lines: Vec<&str> = source.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        if !opens_unsafe_block(code) {
            continue;
        }
        let justified = lines[..index]
            .iter()
            .rev()
            .map(|line| line.trim())
            .take_while(|line| line.starts_with("//"))
            .any(|line| line.trim_start_matches('/').trim().starts_with("SAFETY:"));
        if !justified {
            found.push(index + 1);
        }
    }
    found
}

/// Returns whether a line of code contains the keyword `unsafe` followed by a block.
fn opens_unsafe_block(code: &str) -> bool {
    code.match_indices("unsafe").any(|(start, keyword)| {
        let before = code[..start].chars().next_back();
        let after = code[start + keyword.len()..].trim_start();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_') && after.starts_with('{')
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{check, forbids_unsafe, unjustified_unsafe_blocks};
    use crate::workspace::Member;
    use crate::workspace::tests::scratch;

    #[test]
    fn mg_req_0170_crate_root_must_forbid_unsafe() {
        assert!(forbids_unsafe("//! Docs.\n\n#![forbid(unsafe_code)]\n"));
        assert!(!forbids_unsafe("//! Docs.\n#![deny(unsafe_code)]\n"));
    }

    #[test]
    fn mg_req_0170_crate_without_forbid_is_reported() {
        let root = scratch("unsafe-forbid");
        let dir = root.join("crates").join("mergus-plan");
        fs::create_dir_all(dir.join("src").join("bin")).unwrap();
        fs::write(dir.join("src/lib.rs"), "#![forbid(unsafe_code)]\n").unwrap();
        fs::write(dir.join("src/bin/tool.rs"), "fn main() {}\n").unwrap();
        let member = Member {
            name: "mergus-plan".to_owned(),
            dir: dir.clone(),
        };
        let failure = format!("{:?}", check(&root, &[member]).unwrap_err());
        let expected = "crates/mergus-plan/src/bin/tool.rs: missing";
        assert!(failure.contains(expected), "{failure}");
        assert!(!failure.contains("lib.rs"), "{failure}");
    }

    #[test]
    fn mg_req_0170_ffi_unsafe_block_needs_safety_comment() {
        let source = "\
fn a() {
    // SAFETY: the pointer comes from DuckDB and lives as long as `self`.
    unsafe { call() };
    let x = unsafe { call() };
    // A comment that is not a justification.
    unsafe {
        call();
    }
}
unsafe fn declared() {}
// unsafe { not code }
";
        assert_eq!(unjustified_unsafe_blocks(source), [4, 6]);
    }

    #[test]
    fn mg_req_0170_ffi_crate_may_use_unsafe_without_forbid() {
        let root = scratch("unsafe-ffi");
        let dir = root.join("crates").join("mergus-py");
        fs::create_dir_all(dir.join("src")).unwrap();
        let source = "// SAFETY: checked by the caller.\nunsafe { call() }\n";
        fs::write(dir.join("src/lib.rs"), source).unwrap();
        let member = Member {
            name: "mergus-py".to_owned(),
            dir,
        };
        assert!(check(&root, &[member]).is_ok());
    }
}
