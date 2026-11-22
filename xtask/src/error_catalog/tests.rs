use std::fs;
use std::path::PathBuf;

use super::findings;
use crate::workspace::{self, tests::scratch};

/// The rules of the test specification.
const RULES: &str = "# Rules\n\n| Rule | Title | Kind | Phase | File |\n|---|---|---|---|---|\n\
    | MG-REQ-0001 | English | process | 0.1 | [kernel.md](kernel.md) |\n\
    | MG-REQ-0009 | No panics | process | 0.1 | [kernel.md](kernel.md) |\n\
    | MG-REQ-0303 | Names | behavior | 0.2 | [project.md](project.md) |\n\
    | MG-REQ-0304 | Paths | behavior | 0.2 | [project.md](project.md) |\n";

/// The exit codes of the test specification, in the form of `contracts/events.md`.
const EVENTS: &str = "# Events\n\n## 1. Event stream\n\n| Code | Meaning |\n|---|---|\n| 8 | Not an exit code |\n\n\
    ## 2. Exit codes\n\n| Code | Meaning | Retryable |\n|---|---|---|\n| 0 | Success | — |\n\
    | 1 | Project error | No |\n| 3 | Unavailable | Yes |\n| 10–19 | Internal | No |\n\n\
    ### MG-REQ-0262 · Exit codes have fixed meanings\n\n| 9 | Not an exit code |\n";

/// A workspace in a scratch directory, with the test specification.
struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let tree = Self {
            root: scratch(&format!("error-catalog-{name}")),
        };
        tree.write("spec/RULES.md", RULES);
        tree.write("spec/contracts/events.md", EVENTS);
        tree
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Writes the catalog file of `code`, valid when its rules are, with `extra` list lines.
    fn entry(&self, code: &str, rules: &str, extra: &str) {
        self.write(
            &format!("crates/mergus-cli/errors/{code}.md"),
            &entry(code, rules, extra),
        );
    }

    /// Writes a test that emits `code`.
    fn emitted(&self, code: &str) {
        let name = code.to_lowercase().replace('-', "_");
        let text = format!("#[test]\nfn emits() {{\n    let _ = \"{code}\";\n}}\n");
        self.write(&format!("crates/mergus-cli/tests/{name}.rs"), &text);
    }

    fn findings(&self) -> Vec<String> {
        findings(&self.root)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// The findings of item `item` of MG-REQ-1335.
    fn item(&self, item: u8) -> Vec<String> {
        let prefix = format!("error-catalog: MG-REQ-1335.{item}: ");
        self.findings()
            .into_iter()
            .filter(|finding| finding.starts_with(&prefix))
            .collect()
    }
}

fn entry(code: &str, rules: &str, extra: &str) -> String {
    format!(
        "# {code} · Two models have the same name\n\n- Exit code: 1\n- Retryable: no\n\
         - Rules: {rules}\n{extra}\n## Cause\nTwo files.\n\n## Example\n`a.sql` twice.\n\n\
         ## Fix\nRename one.\n"
    )
}

#[test]
fn mg_req_1335_this_repository_passes() {
    let root = workspace::root().unwrap();
    assert_eq!(findings(&root).unwrap().len(), 0);
}

#[test]
fn mg_req_1335_a_valid_catalog_passes() {
    let tree = Tree::new("valid");
    tree.entry("MG-0301", "MG-REQ-0303, MG-REQ-0304", "");
    tree.entry("MG-0302", "MG-REQ-0303", "- Status: retired\n");
    tree.entry("MG-0101", "MG-REQ-0001, MG-REQ-0009", "");
    tree.entry("MG-0303", "MG-REQ-0304", "");
    tree.emitted("MG-0301");
    tree.emitted("MG-0101");
    tree.write(
        "crates/mergus-cli/src/lib.rs",
        "//! Uses MG-0301 and MG-01xx, never MG-REQ-0303.\npub const A: &str = \"MG-0101\";\n\
         #[cfg(test)]\nmod tests {\n    const B: &str = \"MG-0303\";\n}\n",
    );
    tree.write(
        "crates/mergus-cli/src/tests.rs",
        "const C: &str = \"MG-0303\";\n",
    );
    assert_eq!(tree.findings(), Vec::<String>::new());
}

#[test]
fn mg_req_1335_with_no_catalog_only_item_4_applies() {
    let tree = Tree::new("no-catalog");
    fs::remove_dir_all(tree.root.join("spec")).unwrap();
    tree.write(
        "crates/mergus-run/src/lib.rs",
        "pub const A: &str = \"MG-0701\";\n",
    );
    assert_eq!(
        tree.findings(),
        [
            "error-catalog: MG-REQ-1335.4: crates/mergus-run/src/lib.rs: line 1: MG-0701 has no file under crates/mergus-cli/errors/"
        ]
    );
}

#[test]
fn mg_req_1335_1_misnamed_files_and_wrong_titles_fail() {
    let tree = Tree::new("names");
    tree.write(
        "crates/mergus-cli/errors/MG-301.md",
        &entry("MG-0301", "MG-REQ-0303", ""),
    );
    tree.write("crates/mergus-cli/errors/README.md", "# Errors\n");
    tree.write(
        "crates/mergus-cli/errors/old/MG-0302.md",
        &entry("MG-0302", "MG-REQ-0303", ""),
    );
    tree.write(
        "crates/mergus-cli/errors/MG-0303.md",
        &entry("MG-0304", "MG-REQ-0303", ""),
    );
    let untitled =
        entry("MG-0304", "MG-REQ-0303", "").replace("· Two models have the same name", "· ");
    tree.write("crates/mergus-cli/errors/MG-0304.md", &untitled);
    let dash = entry("MG-0305", "MG-REQ-0303", "").replace(" · ", " - ");
    tree.write("crates/mergus-cli/errors/MG-0305.md", &dash);
    tree.entry("MG-0301", "MG-REQ-0303", "");
    for code in ["MG-0301", "MG-0303", "MG-0304", "MG-0305"] {
        tree.emitted(code);
    }
    let first_line =
        "the first line must be `# <code> · <title>` with the file's code and a non-empty title";
    assert_eq!(
        tree.item(1),
        [
            format!("error-catalog: MG-REQ-1335.1: crates/mergus-cli/errors/MG-0303.md: {first_line}"),
            format!("error-catalog: MG-REQ-1335.1: crates/mergus-cli/errors/MG-0304.md: {first_line}"),
            format!("error-catalog: MG-REQ-1335.1: crates/mergus-cli/errors/MG-0305.md: {first_line}"),
            "error-catalog: MG-REQ-1335.1: crates/mergus-cli/errors/MG-301.md: must be named `MG-<dddd>.md`".to_owned(),
            "error-catalog: MG-REQ-1335.1: crates/mergus-cli/errors/README.md: must be named `MG-<dddd>.md`".to_owned(),
            "error-catalog: MG-REQ-1335.1: crates/mergus-cli/errors/old/MG-0302.md: must be named `MG-<dddd>.md`".to_owned(),
        ]
    );
}

#[test]
fn mg_req_1335_2_missing_or_wrong_list_lines_fail() {
    let tree = Tree::new("lists");
    let file = |n: u8| format!("crates/mergus-cli/errors/MG-030{n}.md");
    let changes = [
        ("- Exit code: 1\n", ""),
        ("- Exit code: 1", "- Exit code: 8"),
        ("- Retryable: no", "- Retryable: No"),
        ("- Rules: MG-REQ-0303\n", ""),
        ("MG-REQ-0303", "MG-REQ-0303,MG-REQ-0304"),
        ("MG-REQ-0303", "MG-REQ-0399"),
        ("- Exit code: 1", "- Exit code: 15"),
        ("- Retryable: no\n", ""),
    ];
    for (n, (from, to)) in (1..).zip(changes) {
        let code = format!("MG-030{n}");
        tree.write(&file(n), &entry(&code, "MG-REQ-0303", "").replace(from, to));
        tree.emitted(&code);
    }
    let finding =
        |n: u8, message: &str| format!("error-catalog: MG-REQ-1335.2: {}: {message}", file(n));
    let exit = "lacks the line `- Exit code: <n>` with a code of `contracts/events.md` section 2";
    let retryable = "lacks the line `- Retryable: yes` or `- Retryable: no`";
    let rules = "lacks the line `- Rules: ` followed by rule IDs separated by `, `";
    assert_eq!(
        tree.item(2),
        [
            finding(1, exit),
            finding(2, exit),
            finding(3, retryable),
            finding(4, rules),
            finding(5, rules),
            finding(6, "lists MG-REQ-0399, which is not in `spec/RULES.md`"),
            finding(8, retryable),
        ]
    );
}

#[test]
fn mg_req_1335_2_missing_or_empty_sections_fail() {
    let tree = Tree::new("sections");
    let file = |n: u8| format!("crates/mergus-cli/errors/MG-030{n}.md");
    let base = |n: u8| entry(&format!("MG-030{n}"), "MG-REQ-0303", "");
    tree.write(&file(1), &base(1).replace("## Cause\nTwo files.\n", ""));
    tree.write(&file(2), &base(2).replace("`a.sql` twice.", "  "));
    tree.write(&file(3), &base(3).replace("Rename one.\n", ""));
    tree.write(&file(4), &base(4).replace("## Cause", "### Cause"));
    for n in 1..=4 {
        tree.emitted(&format!("MG-030{n}"));
    }
    let finding = |n: u8, heading: &str, problem: &str| {
        format!(
            "error-catalog: MG-REQ-1335.2: {}: {problem} `{heading}`",
            file(n)
        )
    };
    assert_eq!(
        tree.item(2),
        [
            finding(1, "## Cause", "lacks the heading"),
            finding(2, "## Example", "has no text under the heading"),
            finding(3, "## Fix", "has no text under the heading"),
            finding(4, "## Cause", "lacks the heading"),
        ]
    );
}

#[test]
fn mg_req_1335_3_a_code_outside_the_area_of_its_rules_fails() {
    let tree = Tree::new("areas");
    tree.entry("MG-0401", "MG-REQ-0303", "");
    tree.entry("MG-0301", "MG-REQ-0303, MG-REQ-0001", "");
    tree.entry("MG-0302", "MG-REQ-0001", "");
    tree.entry("MG-0001", "MG-REQ-0009", "");
    tree.entry("MG-0101", "MG-REQ-0303", "");
    for code in ["MG-0401", "MG-0301", "MG-0302", "MG-0001", "MG-0101"] {
        tree.emitted(code);
    }
    let file = |code: &str| format!("crates/mergus-cli/errors/{code}.md");
    let area = |code: &str, rule: &str| {
        format!(
            "error-catalog: MG-REQ-1335.3: {}: {code} lists {rule}, whose area differs",
            file(code)
        )
    };
    let kernel = |code: &str| {
        format!(
            "error-catalog: MG-REQ-1335.3: {}: {code} lists kernel rules only, so it must be in MG-01xx",
            file(code)
        )
    };
    assert_eq!(
        tree.item(3),
        [
            kernel("MG-0001"),
            area("MG-0101", "MG-REQ-0303"),
            area("MG-0301", "MG-REQ-0001"),
            kernel("MG-0302"),
            area("MG-0401", "MG-REQ-0303"),
        ]
    );
}

#[test]
fn mg_req_1335_4_undocumented_or_retired_codes_in_code_fail() {
    let tree = Tree::new("emitted");
    tree.entry("MG-0301", "MG-REQ-0303", "");
    tree.entry("MG-0302", "MG-REQ-0303", "- Status: retired\n");
    tree.emitted("MG-0301");
    tree.write(
        "crates/mergus-project/src/names.rs",
        "const A: &str = \"MG-0301\";\nconst B: &str = \"MG-0302\";\nconst C: &str = \"MG-0303 MG-03031\";\n\
         const D: &str = \"MG-REQ-0303\";\n#[cfg(test)]\nmod tests {\n    const E: &str = \"MG-0309\";\n}\n",
    );
    tree.write(
        "crates/mergus-project/tests/names.rs",
        "const F: &str = \"MG-0308\";\n",
    );
    tree.write(
        "crates/mergus-project/src/tests.rs",
        "const G: &str = \"MG-0307\";\n",
    );
    tree.write("crates/mergus-project/README.md", "MG-0306\n");
    tree.write("xtask/src/lib.rs", "const H: &str = \"MG-0305\";\n");
    let path = "crates/mergus-project/src/names.rs";
    assert_eq!(
        tree.item(4),
        [
            format!("error-catalog: MG-REQ-1335.4: {path}: line 2: MG-0302 is retired"),
            format!(
                "error-catalog: MG-REQ-1335.4: {path}: line 3: MG-0303 has no file under crates/mergus-cli/errors/"
            ),
        ]
    );
}

#[test]
fn mg_req_1335_5_a_code_no_test_emits_fails() {
    let tree = Tree::new("tested");
    for code in ["MG-0301", "MG-0302", "MG-0303", "MG-0304", "MG-0305"] {
        tree.entry(code, "MG-REQ-0303", "");
    }
    tree.entry("MG-0306", "MG-REQ-0303", "- Status: retired\n");
    tree.write(
        "crates/mergus-project/tests/a.rs",
        "const A: &str = \"MG-0301\";\n",
    );
    tree.write(
        "crates/mergus-project/src/tests.rs",
        "const B: &str = \"MG-0302\";\n",
    );
    tree.write(
        "crates/mergus-project/src/lib.rs",
        "const C: &str = \"MG-0304\";\n#[cfg(test)]\nmod tests {\n    const D: &str = \"MG-0303\";\n}\n",
    );
    tree.write("xtask/tests/a.rs", "const E: &str = \"MG-0305\";\n");
    let finding = |code: &str| {
        format!(
            "error-catalog: MG-REQ-1335.5: crates/mergus-cli/errors/{code}.md: {code} is written in no test source"
        )
    };
    assert_eq!(tree.item(5), [finding("MG-0304"), finding("MG-0305")]);
}

#[test]
fn mg_req_1335_6_a_gap_in_an_area_fails() {
    let tree = Tree::new("gaps");
    for code in [
        "MG-0301", "MG-0302", "MG-0305", "MG-0402", "MG-0500", "MG-0501",
    ] {
        tree.entry(code, "MG-REQ-0303", "");
        tree.emitted(code);
    }
    let gap = |code: &str| {
        format!(
            "error-catalog: MG-REQ-1335.6: crates/mergus-cli/errors/: {code} is missing, so the codes of its area have a gap"
        )
    };
    assert_eq!(
        tree.item(6),
        [
            gap("MG-0303"),
            gap("MG-0304"),
            gap("MG-0401"),
            "error-catalog: MG-REQ-1335.6: crates/mergus-cli/errors/MG-0500.md: MG-0500 is below MG-0501, where the codes of its area start".to_owned(),
        ]
    );
}
