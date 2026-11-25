//! The line of a key in the text of a source file.

use toml::Spanned;
use toml::de::{DeTable, DeValue};

/// A source file parsed again, only to find the line of its keys.
pub(crate) struct Lines<'a> {
    /// The byte offsets where each line starts.
    starts: Vec<usize>,
    /// The parsed file; `None` when the text is not TOML, which `mergus-project` reports.
    root: Option<Spanned<DeTable<'a>>>,
}

impl<'a> Lines<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let breaks = text.match_indices('\n').map(|(offset, _)| offset + 1);
        Self {
            starts: std::iter::once(0).chain(breaks).collect(),
            root: DeTable::parse(text).ok(),
        }
    }

    /// The line, from 1, of the deepest key of `keys`, a path from the root of the file, that
    /// the file holds; 1 when it holds none.
    pub(crate) fn of(&self, keys: &[&str]) -> usize {
        let Some(root) = &self.root else {
            return 1;
        };
        let mut table = root.get_ref();
        let mut offset = None;
        for key in keys {
            let Some((name, value)) = table.iter().find(|(name, _)| name.get_ref() == key) else {
                break;
            };
            offset = Some(name.span().start);
            match value.get_ref() {
                DeValue::Table(inner) => table = inner,
                _ => break,
            }
        }
        offset.map_or(1, |offset| {
            self.starts.partition_point(|start| *start <= offset)
        })
    }
}
