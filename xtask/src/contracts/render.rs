//! Writes the Rust source of the generated contract types, formatted by `rustfmt`.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use super::model::{Field, Item, Type};
use crate::failure::Failure;

/// The command that regenerates the files, named in their headers.
const GENERATE: &str = "cargo xtask contracts-generate";

/// The words Rust reserves, which a field name must escape as `r#<word>`.
const KEYWORDS: [&str; 51] = [
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// One contract version to render.
pub(crate) struct Module {
    /// The module name, `<contract>_v<version>`.
    pub(crate) name: String,
    pub(crate) contract: String,
    pub(crate) version: u32,
    /// The schema path shown in the header, relative to the workspace root.
    pub(crate) shown_schema: String,
    /// The schema path relative to the generated file, for `include_str!`.
    pub(crate) include: String,
    pub(crate) items: Vec<Item>,
}

impl Module {
    /// The name of the root struct, which implements `Contract`.
    pub(crate) fn root(&self) -> &str {
        match self.items.first() {
            Some(Item::Struct { name, .. } | Item::Enum { name, .. }) => name,
            None => "",
        }
    }
}

/// Renders the file of one contract version.
pub(crate) fn module(module: &Module) -> String {
    let mut out = format!(
        "//! The `{}` contract, version {}, generated from `{}` by `{GENERATE}`.\n\
         //! Do not edit.\n\nuse serde::{{Deserialize, Serialize}};\n",
        module.contract, module.version, module.shown_schema
    );
    for item in &module.items {
        out.push('\n');
        match item {
            Item::Struct { name, doc, fields } => render_struct(&mut out, name, doc, fields),
            Item::Enum { name, doc, values } => render_enum(&mut out, name, doc, values),
        }
    }
    let _ = write!(
        out,
        "\nimpl crate::Contract for {} {{\n\
         const NAME: &'static str = {:?};\n\
         const VERSION: u32 = {};\n\
         const SCHEMA: &'static str = include_str!({:?});\n}}\n",
        module.root(),
        module.contract,
        module.version,
        module.include
    );
    out
}

/// Renders `mod.rs`: the modules and the registry of every contract.
pub(crate) fn registry(shown_schemas: &str, modules: &[Module]) -> String {
    let mut out = format!(
        "//! Contract types generated from `{shown_schemas}/` by `{GENERATE}`.\n\
         //! Do not edit.\n\n"
    );
    for module in modules {
        let _ = writeln!(out, "pub mod {};", module.name);
    }
    if !modules.is_empty() {
        out.push('\n');
    }
    out.push_str(
        "/// Every contract that has a schema, sorted by name and version.\n\
         pub const CONTRACTS: &[crate::ContractSchema] = &[",
    );
    for module in modules {
        let _ = write!(
            out,
            "crate::ContractSchema::of::<{}::{}>(),",
            module.name,
            module.root()
        );
    }
    out.push_str("];\n");
    out
}

fn render_doc(out: &mut String, doc: &str) {
    for line in doc.lines() {
        if line.is_empty() {
            out.push_str("///\n");
        } else {
            let _ = writeln!(out, "/// {line}");
        }
    }
}

fn render_struct(out: &mut String, name: &str, doc: &str, fields: &[Field]) {
    render_doc(out, doc);
    let _ = writeln!(
        out,
        "#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n\
         #[serde(deny_unknown_fields)]\npub struct {name} {{"
    );
    for field in fields {
        render_doc(out, &field.doc);
        let ty = rust_type(&field.ty);
        let ident = if KEYWORDS.contains(&field.name.as_str()) {
            format!("r#{}", field.name)
        } else {
            field.name.clone()
        };
        if field.required {
            let _ = writeln!(out, "pub {ident}: {ty},");
        } else {
            let _ = writeln!(
                out,
                "#[serde(default, skip_serializing_if = \"Option::is_none\")]\n\
                 pub {ident}: Option<{ty}>,"
            );
        }
    }
    out.push_str("}\n");
}

fn render_enum(out: &mut String, name: &str, doc: &str, values: &[(String, String)]) {
    render_doc(out, doc);
    let _ = writeln!(
        out,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]\npub enum {name} {{"
    );
    for (value, variant) in values {
        let _ = writeln!(
            out,
            "/// `{value}`.\n#[serde(rename = {value:?})]\n{variant},"
        );
    }
    out.push_str("}\n");
}

fn rust_type(ty: &Type) -> String {
    match ty {
        Type::String => "String".to_owned(),
        Type::Integer => "i64".to_owned(),
        Type::Number => "f64".to_owned(),
        Type::Boolean => "bool".to_owned(),
        Type::List(item) => format!("Vec<{}>", rust_type(item)),
        Type::Named(name) => name.clone(),
    }
}

/// Formats Rust source with `rustfmt` and the workspace's `rustfmt.toml`, as `cargo fmt`
/// does.
///
/// # Errors
///
/// Fails if `rustfmt` cannot run or rejects the source.
pub(crate) fn format(root: &Path, source: &str) -> Result<String, Failure> {
    let config = root.join("rustfmt.toml");
    let mut child = Command::new("rustfmt")
        .arg("--edition")
        .arg("2024")
        .arg("--config-path")
        .arg(&config)
        .arg("--emit")
        .arg("stdout")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Failure::new(format!("cannot run rustfmt: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(source.as_bytes())
            .map_err(|error| Failure::new(format!("cannot write to rustfmt: {error}")))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| Failure::new(format!("rustfmt failed: {error}")))?;
    if !output.status.success() {
        return Err(Failure::new(format!(
            "rustfmt rejected the generated source: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout).map_err(|_| Failure::new("rustfmt printed non-UTF-8"))
}
