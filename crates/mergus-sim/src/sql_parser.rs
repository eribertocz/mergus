//! A simulated SQL parser: trees recorded from the real one (MG-REQ-0193).

use std::collections::BTreeMap;
use std::sync::Mutex;

use mergus_ports::{PortError, SqlParser};

use crate::rng::lock;

/// Returns the tree recorded for each SQL text, and fails with [`PortError::NotFound`] for a
/// text that has none.
#[derive(Debug, Default)]
pub struct SimSqlParser {
    trees: Mutex<BTreeMap<String, String>>,
    select_lists: Mutex<BTreeMap<String, String>>,
}

impl SimSqlParser {
    /// Returns a parser with no recorded tree.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `tree`, the output of the real parser for `sql`.
    pub fn record(&self, sql: impl Into<String>, tree: impl Into<String>) {
        lock(&self.trees).insert(sql.into(), tree.into());
    }

    /// Records `tree`, the output of the real parser's
    /// [`SqlParser::parse_select_list`] for `expression`.
    pub fn record_select_list(&self, expression: impl Into<String>, tree: impl Into<String>) {
        lock(&self.select_lists).insert(expression.into(), tree.into());
    }
}

impl SqlParser for SimSqlParser {
    fn parse(&self, sql: &str) -> Result<String, PortError> {
        lock(&self.trees)
            .get(sql)
            .cloned()
            .ok_or_else(|| PortError::NotFound(format!("a recorded tree for `{sql}`")))
    }

    fn parse_select_list(&self, expression: &str) -> Result<String, PortError> {
        lock(&self.select_lists)
            .get(expression)
            .cloned()
            .ok_or_else(|| {
                PortError::NotFound(format!("a recorded select-list tree for `{expression}`"))
            })
    }
}
