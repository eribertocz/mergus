//! The kinds (MG-REQ-0401) and modes (MG-REQ-0409) of a source.

use std::fmt;

/// The kind of a source (MG-REQ-0401).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceKind {
    /// A database, read through ADBC.
    Database,
    /// Parquet, CSV or JSON files on a file system or an object store.
    Files,
    /// A REST API (`extract/rest.md`).
    Rest,
}

impl SourceKind {
    /// Reads the `kind` of a source file; `None` when it is missing or not one of the three,
    /// which the schema of `mergus-project` already reports as `MG-0305`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "database" => Some(Self::Database),
            "files" => Some(Self::Files),
            "rest" => Some(Self::Rest),
            _ => None,
        }
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Database => "database",
            Self::Files => "files",
            Self::Rest => "rest",
        })
    }
}

/// How a source is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mode {
    /// Extracted into raw, the default.
    Copy,
    /// Read in place, pinned by version.
    Connect,
    /// Queried by the executor at read time.
    Live,
}

impl Mode {
    /// Reads the `mode` of a source file; `None` when it is not one of the three.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "copy" => Some(Self::Copy),
            "connect" => Some(Self::Connect),
            "live" => Some(Self::Live),
            _ => None,
        }
    }

    /// The engines a `connect` or `live` source may declare (MG-REQ-0409.1, MG-REQ-0409.2);
    /// none for `copy`, whose engines depend on the kind ([`Mode::engines_for`]).
    #[must_use]
    pub fn engines(self) -> &'static [&'static str] {
        match self {
            Self::Copy => &[],
            Self::Connect => &["ducklake", "iceberg", "delta"],
            Self::Live => &[
                "duckdb",
                "ducklake",
                "clickhouse",
                "iceberg",
                "delta",
                "parquet",
            ],
        }
    }

    /// The engines a source of `kind` read in this mode must declare one of; empty when it
    /// must not declare `engine`. A `copy` source of `kind = "database"` names the database
    /// Mergus reads, `postgresql` or `duckdb`, and a `copy` source of another kind declares no
    /// engine (MG-REQ-0409.4).
    #[must_use]
    pub fn engines_for(self, kind: SourceKind) -> &'static [&'static str] {
        match (self, kind) {
            (Self::Copy, SourceKind::Database) => &["postgresql", "duckdb"],
            (mode, _) => mode.engines(),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Copy => "copy",
            Self::Connect => "connect",
            Self::Live => "live",
        })
    }
}
