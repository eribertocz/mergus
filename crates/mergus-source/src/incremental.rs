//! An incremental read of one table whose position moves only once the caller has committed
//! each chunk (MG-REQ-0412, MG-REQ-0413, MG-REQ-0418).

use std::num::NonZeroU32;

use arrow::array::{Array, AsArray, BooleanArray};
use arrow::compute::{filter_record_batch, is_not_null, not};
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use mergus_ports::{PortError, SqlValue};
use mergus_project::Source;

use crate::engine::AttachedSource;
use crate::incremental_error::ExtractError;
use crate::options::{DeletePolicy, LoadOptions, load_options};
use crate::read::{Projection, ReadOptions, last_key, statement};
use crate::schema_change::{SourceColumn, source_columns};
use crate::table::DeclaredTable;
use crate::temporal::Temporal;

/// Where an incremental read starts.
#[derive(Clone, Debug, Default)]
pub struct Start {
    /// The rows of one keyset chunk (MG-REQ-0417); `None`, or a table without `key`, reads
    /// the table in one statement.
    pub chunk_rows: Option<NonZeroU32>,
    /// The `watermark_after` of the table's last recorded load; `None` for a first load.
    pub watermark: Option<String>,
    /// The key of the last row an unfinished load committed, as a batch whose last row holds
    /// the key columns: the read resumes after it (MG-REQ-0418.2).
    pub resume_after: Option<RecordBatch>,
}

/// The rows of one chunk.
#[derive(Clone, Debug)]
pub struct Chunk {
    /// The rows read that are not deleted.
    pub rows: Vec<RecordBatch>,
    /// The rows the delete column marks deleted (MG-REQ-0415.2), with every column.
    pub deleted: Vec<RecordBatch>,
    /// Whether it is the last chunk of the read.
    pub last: bool,
}

/// The position a chunk read, applied once it is committed.
#[derive(Clone, Debug)]
struct Pending {
    after: Option<Vec<SqlValue>>,
    last: bool,
}

/// An incremental read of one table.
pub struct IncrementalReader<'a> {
    pub(crate) attached: &'a mut AttachedSource,
    pub(crate) source_name: String,
    pub(crate) table: DeclaredTable,
    pub(crate) options: LoadOptions,
    pub(crate) columns: Vec<SourceColumn>,
    pub(crate) read: ReadOptions,
    after: Option<Vec<SqlValue>>,
    pending: Option<Pending>,
    done: bool,
}

impl std::fmt::Debug for IncrementalReader<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IncrementalReader")
            .field("table", &self.table)
            .field("since", &self.read.since)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}

impl<'a> IncrementalReader<'a> {
    /// Opens the read of every column of `table` of `source` from `attached`. The table's
    /// columns are read first with `DESCRIBE`. With a watermark, the read starts at
    /// `watermark − overlap` for a `DATE`, `TIMESTAMP` or `TIMESTAMPTZ` incremental column,
    /// and at the watermark for any other (MG-REQ-0412.2).
    ///
    /// # Errors
    ///
    /// - [`ExtractError::MissingDeleteColumn`] when the table declares `column:<name>` and
    ///   the source has no such column (MG-REQ-0415.2);
    /// - [`ExtractError::DeletesNotRunnable`] for `compare` without `key`, or `delete_window`
    ///   without an incremental column of a date or time type (MG-REQ-0415.4);
    /// - [`ExtractError::Port`] when a statement fails, the watermark is not a value of the
    ///   incremental column's type, or `resume_after` lacks a key column.
    pub fn open(
        attached: &'a mut AttachedSource,
        source: &Source,
        table: &DeclaredTable,
        start: Start,
    ) -> Result<Self, ExtractError> {
        let options = load_options(source, table);
        let columns = source_columns(attached, table)?;
        if let DeletePolicy::Column(column) = &options.deletes
            && !columns.iter().any(|known| &known.name == column)
        {
            return Err(ExtractError::MissingDeleteColumn {
                source_name: source.name.clone(),
                table: table.name.clone(),
                column: column.clone(),
            });
        }
        refuse_unrunnable(source, table, &options, &columns)?;
        let since = match (&table.incremental, &start.watermark) {
            (Some(column), Some(watermark)) => {
                Some(read_since(&columns, column, watermark, &options)?)
            }
            _ => None,
        };
        let after = match &start.resume_after {
            Some(batch) if !table.key.is_empty() => {
                Some(last_key(std::slice::from_ref(batch), &table.key)?)
            }
            _ => None,
        };
        Ok(Self {
            attached,
            source_name: source.name.clone(),
            table: table.clone(),
            options,
            columns,
            read: ReadOptions {
                projection: Projection::All,
                since: since.map(SqlValue::Text),
                chunk_rows: start.chunk_rows,
            },
            after,
            pending: None,
            done: false,
        })
    }

    /// The load options of the table.
    #[must_use]
    pub fn options(&self) -> &LoadOptions {
        &self.options
    }

    /// The columns of the table in the source, from `DESCRIBE`.
    #[must_use]
    pub fn columns(&self) -> &[SourceColumn] {
        &self.columns
    }

    /// The lowest incremental value the read keeps, `watermark − overlap`, in text; `None`
    /// for a first load or a table without `incremental`.
    #[must_use]
    pub fn since(&self) -> Option<&str> {
        match &self.read.since {
            Some(SqlValue::Text(since)) => Some(since),
            _ => None,
        }
    }

    /// Reads the chunk after the last committed one, or `None` once the last chunk is
    /// committed. Until [`committed`](Self::committed) is called, the next call reads the
    /// same chunk again (MG-REQ-0418).
    ///
    /// # Errors
    ///
    /// Fails with [`ExtractError::Port`] when the statement fails.
    pub fn next_chunk(&mut self) -> Result<Option<Chunk>, ExtractError> {
        if self.done {
            return Ok(None);
        }
        let chunk_rows = self.read.chunk_rows.filter(|_| !self.table.key.is_empty());
        let (sql, params) = statement(
            &self.table,
            &self.read,
            &self.attached.alias,
            chunk_rows,
            self.after.as_deref(),
        );
        let batches = self.attached.connection.query_arrow(&sql, &params)?;
        let read: usize = batches.iter().map(RecordBatch::num_rows).sum();
        let last = chunk_rows.is_none_or(|limit| read < limit.get() as usize);
        let after = if last {
            None
        } else {
            Some(last_key(&batches, &self.table.key)?)
        };
        self.pending = Some(Pending { after, last });
        let (rows, deleted) = self.split(batches)?;
        Ok(Some(Chunk {
            rows,
            deleted,
            last,
        }))
    }

    /// Records that the caller committed the chunk read last: only now does the read move
    /// past it (MG-REQ-0413, MG-REQ-0418).
    pub fn committed(&mut self) {
        if let Some(pending) = self.pending.take() {
            self.done = pending.last;
            if pending.after.is_some() {
                self.after = pending.after;
            }
        }
    }

    /// The rows of `batches` the delete column keeps and those it marks deleted: `true`, or
    /// any non-NULL value of a non-boolean column (MG-REQ-0415.2).
    fn split(
        &self,
        batches: Vec<RecordBatch>,
    ) -> Result<(Vec<RecordBatch>, Vec<RecordBatch>), PortError> {
        let DeletePolicy::Column(column) = &self.options.deletes else {
            return Ok((batches, Vec::new()));
        };
        let failed = |error: arrow::error::ArrowError| PortError::Failed(error.to_string());
        let mut rows = Vec::new();
        let mut deleted = Vec::new();
        for batch in batches {
            let flag = batch.column_by_name(column).ok_or_else(|| {
                PortError::Failed(format!("the rows have no delete column `{column}`"))
            })?;
            let marked: BooleanArray = if flag.data_type() == &DataType::Boolean {
                flag.as_boolean()
                    .iter()
                    .map(|value| Some(value == Some(true)))
                    .collect()
            } else {
                is_not_null(flag.as_ref()).map_err(failed)?
            };
            let kept = not(&marked).map_err(failed)?;
            rows.push(filter_record_batch(&batch, &kept).map_err(failed)?);
            deleted.push(filter_record_batch(&batch, &marked).map_err(failed)?);
        }
        Ok((rows, deleted))
    }
}

/// The start of a read from `watermark` on the incremental `column` (MG-REQ-0412.2).
fn read_since(
    columns: &[SourceColumn],
    column: &str,
    watermark: &str,
    options: &LoadOptions,
) -> Result<String, PortError> {
    let temporal = columns
        .iter()
        .find(|known| known.name == column)
        .and_then(|known| Temporal::of(&known.data_type));
    match temporal {
        None => Ok(watermark.to_owned()),
        Some(temporal) => temporal.minus(watermark, options.overlap).ok_or_else(|| {
            PortError::Failed(format!(
                "the watermark `{watermark}` is not a value of the column `{column}`"
            ))
        }),
    }
}

/// Refuses `compare` on a table without `key`, and `delete_window` on a table whose
/// incremental column is missing or not a `DATE`, `TIMESTAMP` or `TIMESTAMPTZ`
/// (MG-REQ-0415.4).
fn refuse_unrunnable(
    source: &Source,
    table: &DeclaredTable,
    options: &LoadOptions,
    columns: &[SourceColumn],
) -> Result<(), ExtractError> {
    let refuse = |option, reason: &str| ExtractError::DeletesNotRunnable {
        source_name: source.name.clone(),
        table: table.name.clone(),
        option,
        reason: reason.to_owned(),
    };
    if options.deletes == DeletePolicy::Compare && table.key.is_empty() {
        return Err(refuse("deletes", "it has no `key` to compare"));
    }
    if options.delete_window.is_none() {
        return Ok(());
    }
    let temporal = table.incremental.as_ref().and_then(|column| {
        columns
            .iter()
            .find(|known| &known.name == column)
            .and_then(|known| Temporal::of(&known.data_type))
    });
    match temporal {
        Some(_) => Ok(()),
        None => Err(refuse(
            "delete_window",
            "its incremental column is missing or not a date or time column",
        )),
    }
}
