//! What an adapter declares it implements.

use std::fmt;

use arrow::datatypes::DataType;

/// How the storage keeps data versions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Versioning {
    /// The storage format versions data itself.
    Native,
    /// Versions are kept as files.
    Files,
    /// The storage keeps no versions.
    #[default]
    None,
}

/// How [`write`](crate::StorageAdapter::write) applies a stream to a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WriteMode {
    /// Adds the rows to the table.
    Append,
    /// Replaces the whole table with the rows.
    Replace,
    /// Replaces the rows of one period with the rows.
    ReplacePeriod,
    /// Merges the rows into the table by key.
    MergeByKey,
}

impl fmt::Display for WriteMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Append => "append",
            Self::Replace => "replace",
            Self::ReplacePeriod => "replace period",
            Self::MergeByKey => "merge by key",
        })
    }
}

/// What an adapter declares it implements (MG-REQ-0287). The default declares nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// How data versions are kept.
    pub versioning: Versioning,
    /// Whether one commit can span several tables.
    pub multi_table_transactions: bool,
    /// Whether several clients can write at the same time.
    pub concurrent_clients: bool,
    /// The Arrow types the storage stores exactly.
    pub exact_types: Vec<DataType>,
    /// The write modes the adapter implements.
    pub write_modes: Vec<WriteMode>,
}
