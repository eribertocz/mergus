//! The tables extracted into raw: those of a `copy` source only (MG-REQ-0405).

use mergus_project::Source;

use crate::kind::Mode;
use crate::table::{DeclaredTable, declared_tables};

/// Returns the declared tables of `source` that are copied into raw, in the order of their
/// names: every declared table of a `copy` source, the default mode, and none of a `connect`
/// or `live` source, which is read in place (MG-REQ-0405).
#[must_use]
pub fn copied_tables(source: &Source) -> Vec<DeclaredTable> {
    let mode = match source.file.value.get("mode") {
        None => Some(Mode::Copy),
        Some(mode) => mode.as_str().and_then(Mode::parse),
    };
    if mode == Some(Mode::Copy) {
        declared_tables(source)
    } else {
        Vec::new()
    }
}
