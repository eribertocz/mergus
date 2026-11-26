//! The extraction of a source with the read time of each table (MG-REQ-0419) and its
//! breaking changes checked first (MG-REQ-0407).
//!
//! Through the DuckDB `postgres` extension no official page documents a snapshot shared by
//! several attached tables, so every source records the start and the end of each table's
//! read, and the spread of the source (MG-REQ-0419.3).

use std::time::Duration;

use mergus_ports::{Clock, PortError, Timestamp};
use mergus_project::Source;

use crate::copied::copied_tables;
use crate::engine::AttachedSource;
use crate::read::{ReadOptions, TableExtraction, read_table};
use crate::schema_change::{SchemaChange, SourceColumn, breaking_changes, source_columns};
use crate::table::DeclaredTable;

/// The read of one table: its rows, its columns and when it ran.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRead {
    /// The rows read and the raw tables they land in.
    pub extraction: TableExtraction,
    /// The columns of the table in the source, to compare at the next load (MG-REQ-0407).
    pub columns: Vec<SourceColumn>,
    /// The start of its first statement, in UTC (MG-REQ-0419.1).
    pub started: Timestamp,
    /// The end of its last statement, in UTC (MG-REQ-0419.1).
    pub finished: Timestamp,
}

/// The extraction of a source: the tables read and the tables stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceExtraction {
    /// The tables read, in the order of their names.
    pub reads: Vec<TableRead>,
    /// The tables whose extraction stopped on a breaking change, in the order of their names
    /// (MG-REQ-0407).
    pub stopped: Vec<SchemaChange>,
}

impl SourceExtraction {
    /// Returns the latest end minus the earliest start of the reads of the source
    /// (MG-REQ-0419.2), zero when the wall clock went backwards; `None` when no table was
    /// read.
    #[must_use]
    pub fn spread(&self) -> Option<Duration> {
        let start = self.reads.iter().map(|read| read.started).min()?;
        let end = self.reads.iter().map(|read| read.finished).max()?;
        let nanos = end.unix_nanos().saturating_sub(start.unix_nanos());
        Some(u64::try_from(nanos).map_or(Duration::ZERO, Duration::from_nanos))
    }
}

/// Reads every table `source` copies (MG-REQ-0405) from `attached`, in the order of their
/// names, each with the options `options` gives for it. Before a table is read its columns
/// are compared with `last_columns`, those of its last load; a breaking change stops that
/// table only, and nothing of it is read (MG-REQ-0407). The start and the end of each read
/// are taken from `clock` (MG-REQ-0419).
///
/// # Errors
///
/// Fails with the first [`PortError`] of a query.
pub fn extract_source_timed(
    attached: &mut AttachedSource,
    source: &Source,
    options: &dyn Fn(&DeclaredTable) -> ReadOptions,
    last_columns: &dyn Fn(&DeclaredTable) -> Vec<SourceColumn>,
    clock: &dyn Clock,
) -> Result<SourceExtraction, PortError> {
    let mut extraction = SourceExtraction {
        reads: Vec::new(),
        stopped: Vec::new(),
    };
    for table in copied_tables(source) {
        let columns = source_columns(attached, &table)?;
        let changes = breaking_changes(&last_columns(&table), &columns);
        if !changes.is_empty() {
            extraction.stopped.push(SchemaChange {
                source_name: source.name.clone(),
                table: table.name,
                changes,
            });
            continue;
        }
        let table_options = options(&table);
        let started = clock.now();
        let chunks = read_table(attached, &table, &table_options).collect::<Result<Vec<_>, _>>()?;
        let finished = clock.now();
        extraction.reads.push(TableRead {
            extraction: TableExtraction {
                raw: table.raw(&source.name),
                table: table.name,
                batches: chunks.concat(),
            },
            columns,
            started,
            finished,
        });
    }
    Ok(extraction)
}
