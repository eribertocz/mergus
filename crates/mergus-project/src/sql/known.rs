//! The tables a `.sql` model may read (MG-REQ-0373).

use std::collections::BTreeSet;

use serde_json::Value;

use crate::{Configuration, Layout};

/// The schema of the raw tables of sources and seeds (MG-REQ-0373.2).
const RAW: &str = "raw";

/// The tables a `.sql` model may read: the models of the project, and in schema `raw` the
/// tables of the declared sources and the seeds (MG-REQ-0373). Every name is lowercase.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KnownTables {
    models: BTreeSet<String>,
    raw: BTreeSet<String>,
}

impl KnownTables {
    /// Returns the tables of the models of `layout`, of the `[tables.<table>]` of each
    /// source of `configuration` and of the `[seeds.<name>]` of its `project.toml`.
    #[must_use]
    pub fn new(layout: &Layout, configuration: &Configuration) -> Self {
        let mut raw = BTreeSet::new();
        for source in &configuration.sources {
            for table in keys(&source.file.value, "tables") {
                raw.insert(format!("{}__{table}", source.name).to_lowercase());
            }
        }
        for seed in keys(&configuration.project.value, "seeds") {
            raw.insert(format!("seed__{seed}").to_lowercase());
        }
        Self {
            models: layout
                .models
                .iter()
                .map(|model| model.name.to_lowercase())
                .collect(),
            raw,
        }
    }

    /// Returns whether the reference `schema.table` (`schema` empty when the reference has
    /// none) resolves, compared without case. A reference with a catalog never resolves.
    #[must_use]
    pub fn resolves(&self, catalog: &str, schema: &str, table: &str) -> bool {
        let table = table.to_lowercase();
        match (catalog.is_empty(), schema.to_lowercase().as_str()) {
            (true, "") => self.models.contains(&table),
            (true, RAW) => self.raw.contains(&table),
            _ => false,
        }
    }

    /// The models, lowercase, sorted.
    pub(crate) fn models(&self) -> &BTreeSet<String> {
        &self.models
    }

    /// The raw tables of sources and seeds without their schema, lowercase, sorted.
    pub(crate) fn raw_tables(&self) -> &BTreeSet<String> {
        &self.raw
    }

    /// Every known table as a reference names it: `<model>` or `raw.<table>`, sorted.
    pub(crate) fn names(&self) -> impl Iterator<Item = String> + '_ {
        self.models
            .iter()
            .cloned()
            .chain(self.raw.iter().map(|table| format!("{RAW}.{table}")))
    }
}

/// The keys of the table `key` of the configuration `value`, when it is one.
fn keys<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a String> {
    value
        .get(key)
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(serde_json::Map::keys)
}
