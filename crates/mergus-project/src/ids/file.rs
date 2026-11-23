//! `ids.toml`: the stored renames of MG-REQ-0370.

use std::collections::BTreeMap;
use std::path::Path;

use mergus_ports::FileSystem;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use super::{IdKind, Identifier};
use crate::{LayoutError, ProjectError};

/// The path of the file, relative to the project root.
const IDS_FILE: &str = "ids.toml";

/// The entries of `ids.toml`: each current name mapped to the identifier it keeps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Renames {
    models: BTreeMap<String, Identifier>,
    sources: BTreeMap<String, Identifier>,
}

impl Renames {
    /// Parses the text of `ids.toml`, read from `path`.
    ///
    /// # Errors
    ///
    /// Fails with every problem of MG-REQ-0370.1 and MG-REQ-0370.3, in key order.
    pub fn parse(path: &str, text: &str) -> Result<Self, Vec<ProjectError>> {
        let mut parser = Parser {
            path,
            starts: line_starts(text),
            renames: Self::default(),
            owners: BTreeMap::new(),
            problems: Vec::new(),
        };
        match DeTable::parse(text) {
            Ok(root) => parser.root(root.get_ref()),
            Err(error) => {
                let line = error.span().map_or(1, |span| parser.line(span.start));
                parser.problem(line, None, error.message().to_owned());
            }
        }
        if parser.problems.is_empty() {
            Ok(parser.renames)
        } else {
            Err(parser.problems)
        }
    }

    /// Returns the identifier stored for the model `name`.
    #[must_use]
    pub fn model(&self, name: &str) -> Option<&Identifier> {
        self.models.get(name)
    }

    /// Returns the identifier stored for the source `name`.
    #[must_use]
    pub fn source(&self, name: &str) -> Option<&Identifier> {
        self.sources.get(name)
    }

    /// Returns whether `identifier` is a value of the file, and so is never derived
    /// (MG-REQ-0370.4, MG-REQ-0371).
    pub(crate) fn is_reserved(&self, identifier: &Identifier) -> bool {
        self.models
            .values()
            .chain(self.sources.values())
            .any(|stored| stored == identifier)
    }
}

/// Reads `ids.toml` at the root of the project at `root`. A missing file has no entry.
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when the port fails, and with [`LayoutError::Invalid`]
/// carrying every problem of the file.
pub fn read_renames(files: &dyn FileSystem, root: &Path) -> Result<Renames, LayoutError> {
    let path = root.join(IDS_FILE);
    if !files.exists(&path)? {
        return Ok(Renames::default());
    }
    let bytes = files.read(&path)?;
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Err(LayoutError::Invalid(vec![ProjectError::InvalidIds {
            path: IDS_FILE.to_owned(),
            line: 1,
            key: None,
            message: "the file is not UTF-8".to_owned(),
        }]));
    };
    Renames::parse(IDS_FILE, text).map_err(LayoutError::Invalid)
}

/// The state of parsing one `ids.toml`.
struct Parser<'a> {
    path: &'a str,
    starts: Vec<usize>,
    renames: Renames,
    /// The dotted key of each identifier met.
    owners: BTreeMap<Identifier, String>,
    problems: Vec<ProjectError>,
}

impl Parser<'_> {
    /// Reads the top-level tables: only `[model]` and `[source]`.
    fn root(&mut self, root: &DeTable<'_>) {
        for (key, value) in root {
            let name = key.get_ref().to_string();
            let line = self.line(key.span().start);
            let kind = match name.as_str() {
                "model" => IdKind::Model,
                "source" => IdKind::Source,
                _ => {
                    let message = "is an unknown table; the tables are `model` and `source`";
                    self.problem(line, Some(name), message.to_owned());
                    continue;
                }
            };
            if let DeValue::Table(table) = value.get_ref() {
                self.table(kind, &name, table);
            } else {
                self.problem(line, Some(name), "is not a table".to_owned());
            }
        }
    }

    /// Reads the entries of `[model]` or `[source]`.
    fn table(&mut self, kind: IdKind, table_name: &str, table: &DeTable<'_>) {
        for (key, value) in table {
            let name = key.get_ref().to_string();
            let dotted = format!("{table_name}.{name}");
            let line = self.line(key.span().start);
            let Some(identifier) = identifier(kind, value) else {
                let message = format!(
                    "is not a {table_name} identifier: a string `{}_` and 16 lowercase base32 \
                     characters",
                    kind.prefix()
                );
                self.problem(line, Some(dotted), message);
                continue;
            };
            if let Some(first) = self.owners.get(&identifier) {
                self.problems.push(ProjectError::DuplicateIdsEntry {
                    path: self.path.to_owned(),
                    line,
                    identifier: identifier.to_string(),
                    first: first.clone(),
                    second: dotted,
                });
                continue;
            }
            self.owners.insert(identifier.clone(), dotted);
            let entries = match kind {
                IdKind::Source => &mut self.renames.sources,
                _ => &mut self.renames.models,
            };
            entries.insert(name, identifier);
        }
    }

    /// Records a problem of MG-REQ-0370.1.
    fn problem(&mut self, line: usize, key: Option<String>, message: String) {
        self.problems.push(ProjectError::InvalidIds {
            path: self.path.to_owned(),
            line,
            key,
            message,
        });
    }

    /// Returns the line, from 1, of the byte `offset`.
    fn line(&self, offset: usize) -> usize {
        self.starts.partition_point(|start| *start <= offset)
    }
}

/// Returns the identifier of an entry when it is a string valid under MG-REQ-0368 with the
/// prefix of `kind`.
fn identifier(kind: IdKind, value: &Spanned<DeValue<'_>>) -> Option<Identifier> {
    match value.get_ref() {
        DeValue::String(text) => Identifier::parse(text).filter(|found| found.kind() == kind),
        _ => None,
    }
}

/// Returns the byte offsets where each line of `text` starts.
fn line_starts(text: &str) -> Vec<usize> {
    let breaks = text.match_indices('\n').map(|(offset, _)| offset + 1);
    std::iter::once(0).chain(breaks).collect()
}
