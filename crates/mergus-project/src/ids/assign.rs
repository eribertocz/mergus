//! Giving each object of a project its identifier, and checking that none is shared
//! (MG-REQ-0371).

use std::collections::BTreeMap;

use super::{IdKind, Identifier, Renames, derive_identifier};
use crate::ProjectError;

/// The objects met so far.
struct Owner {
    /// How errors name the object: `model `fact_sales``, `table `orders` of source `erp``.
    description: String,
    /// The path of the file that declares it.
    path: String,
}

/// Gives identifiers to the objects of one project, from the entries of its `ids.toml` or
/// derived from their names, and collects the identifiers given twice.
pub struct Identifiers<'a> {
    renames: &'a Renames,
    owners: BTreeMap<Identifier, Owner>,
    problems: Vec<ProjectError>,
}

impl<'a> Identifiers<'a> {
    /// Starts giving identifiers with the entries of `renames`.
    #[must_use]
    pub fn new(renames: &'a Renames) -> Self {
        Self {
            renames,
            owners: BTreeMap::new(),
            problems: Vec::new(),
        }
    }

    /// Returns the identifier of the model `name`, declared by the file `path`.
    pub fn model(&mut self, name: &str, path: &str) -> Identifier {
        let identifier = match self.renames.model(name) {
            Some(stored) => stored.clone(),
            None => self.derive(IdKind::Model, None, name),
        };
        self.record(identifier, format!("model `{name}`"), path)
    }

    /// Returns the identifier of the source `name`, declared by the file `path`.
    pub fn source(&mut self, name: &str, path: &str) -> Identifier {
        let identifier = match self.renames.source(name) {
            Some(stored) => stored.clone(),
            None => self.derive(IdKind::Source, None, name),
        };
        self.record(identifier, format!("source `{name}`"), path)
    }

    /// Returns the identifier of the table `name` of the source identified by `source`,
    /// declared by the file `path`.
    pub fn source_table(&mut self, source: &Identifier, name: &str, path: &str) -> Identifier {
        let identifier = self.derive(IdKind::SourceTable, Some(source), name);
        let description = format!("table `{name}` of {}", self.describe(source));
        self.record(identifier, description, path)
    }

    /// Returns the identifier of the column `name` of the model or source table identified by
    /// `table`, declared by the file `path`.
    pub fn column(&mut self, table: &Identifier, name: &str, path: &str) -> Identifier {
        let identifier = self.derive(IdKind::Column, Some(table), name);
        let description = format!("column `{name}` of {}", self.describe(table));
        self.record(identifier, description, path)
    }

    /// Ends giving identifiers.
    ///
    /// # Errors
    ///
    /// Fails with one problem for each object whose identifier an earlier object has, in the
    /// order the objects were given (MG-REQ-0371).
    pub fn finish(self) -> Result<(), Vec<ProjectError>> {
        if self.problems.is_empty() {
            Ok(())
        } else {
            Err(self.problems)
        }
    }

    /// Returns the derived identifier of MG-REQ-0369: the first `<n>` from 0 whose result is
    /// not a value of `ids.toml`.
    fn derive(&self, kind: IdKind, parent: Option<&Identifier>, name: &str) -> Identifier {
        let mut n: u64 = 0;
        loop {
            let identifier = derive_identifier(kind, parent, name, n);
            if !self.renames.is_reserved(&identifier) {
                return identifier;
            }
            n = n.saturating_add(1);
        }
    }

    /// Returns how errors name the object identified by `identifier`.
    fn describe(&self, identifier: &Identifier) -> String {
        self.owners.get(identifier).map_or_else(
            || format!("`{identifier}`"),
            |owner| owner.description.clone(),
        )
    }

    /// Records that `identifier` names the object `description` of `path`, or a problem when
    /// an earlier object has it.
    fn record(&mut self, identifier: Identifier, description: String, path: &str) -> Identifier {
        if let Some(first) = self.owners.get(&identifier) {
            self.problems.push(ProjectError::DuplicateIdentifier {
                identifier: identifier.to_string(),
                first: first.description.clone(),
                first_path: first.path.clone(),
                second: description,
                path: path.to_owned(),
            });
        } else {
            let owner = Owner {
                description,
                path: path.to_owned(),
            };
            self.owners.insert(identifier.clone(), owner);
        }
        identifier
    }
}
