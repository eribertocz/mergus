//! The `compare` of a table: keys of raw missing from the source are deletes
//! (MG-REQ-0415.3), and keys of the source missing from raw older than the start of the reads
//! are very late rows (MG-REQ-0420).

use std::collections::BTreeSet;
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, AsArray, BooleanArray, Scalar, StringArray};
use arrow::compute::kernels::cmp::lt;
use arrow::compute::{cast, filter_record_batch};
use arrow::datatypes::{DataType, Schema, TimeUnit, TimestampMicrosecondType};
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use arrow::row::{OwnedRow, RowConverter, SortField};
use mergus_ports::{PortError, SqlValue, Timestamp};

use crate::incremental::IncrementalReader;
use crate::incremental_error::{ExtractError, LateRows};
use crate::options::DeletePolicy;
use crate::read::quote_identifier;
use crate::temporal::{Temporal, value_text};

/// What a `compare` found.
#[derive(Clone, Debug, Default)]
pub struct Comparison {
    /// The rows of raw whose key the source no longer holds: the deletes, with their last
    /// known values (MG-REQ-0415.3).
    pub deleted: Vec<RecordBatch>,
    /// The rows of the source missing from raw and older than the start of the reads
    /// (MG-REQ-0420); `None` when there is none.
    pub late: Option<LateRows>,
}

impl IncrementalReader<'_> {
    /// Returns whether a `compare` of the table is due at `now`, its last compare having run
    /// at `last`: the table declares `deletes = "compare"` and a `key`, and its
    /// `delete_check` has come (MG-REQ-0415.3).
    #[must_use]
    pub fn compare_due(&self, last: Option<Timestamp>, now: Timestamp) -> bool {
        self.options.deletes == DeletePolicy::Compare
            && !self.table.key.is_empty()
            && self.options.delete_check.is_due(last, now)
    }

    /// Compares the keys of the source with `raw`, the current rows of the table, at `now`.
    /// It reads only the key columns, and the incremental column when the table has one.
    /// With `delete_window`, both sides keep only the rows whose incremental value is not
    /// older than `now − delete_window` (MG-REQ-0415.3).
    ///
    /// # Errors
    ///
    /// Fails with [`ExtractError::Port`] when the statement fails or the rows cannot be
    /// compared.
    pub fn compare(
        &mut self,
        raw: &[RecordBatch],
        now: Timestamp,
    ) -> Result<Comparison, ExtractError> {
        let failed = |error: ArrowError| PortError::Failed(error.to_string());
        let Some(first) = raw.first() else {
            return Ok(Comparison::default());
        };
        let schema = first.schema();
        let incremental = self.table.incremental.clone();
        let temporal = self.temporal();
        let window = self.window(temporal, now);
        let source_rows = self.source_keys(incremental.as_deref(), window.as_ref())?;
        let keys = Keys::new(&schema, &self.table.key).map_err(failed)?;
        let source_keys = keys.set(&source_rows).map_err(failed)?;
        let raw_keys = keys.set(raw).map_err(failed)?;
        let mut deleted = Vec::new();
        for batch in raw {
            let mut keep = keys
                .of(batch)
                .map_err(failed)?
                .iter()
                .map(|key| !source_keys.contains(key))
                .collect::<Vec<_>>();
            if let (Some((temporal, since)), Some(column)) = (&window, &incremental) {
                let inside = not_older(batch, column, *temporal, since)?;
                keep.iter_mut()
                    .zip(inside)
                    .for_each(|(keep, inside)| *keep &= inside);
            }
            let mask = BooleanArray::from(keep);
            deleted.push(filter_record_batch(batch, &mask).map_err(failed)?);
        }
        let late = match (&incremental, self.since()) {
            (Some(column), Some(since)) => {
                let since = since.to_owned();
                self.late_rows(&source_rows, &keys, &raw_keys, column, temporal, since)?
            }
            _ => None,
        };
        Ok(Comparison { deleted, late })
    }

    /// The temporal kind of the incremental column, `None` without one or for another type.
    fn temporal(&self) -> Option<Temporal> {
        let column = self.table.incremental.as_ref()?;
        self.columns
            .iter()
            .find(|known| &known.name == column)
            .and_then(|known| Temporal::of(&known.data_type))
    }

    /// The start of `delete_window` back from `now`, for a temporal incremental column.
    fn window(&self, temporal: Option<Temporal>, now: Timestamp) -> Option<(Temporal, String)> {
        let (temporal, window) = (temporal?, self.options.delete_window?);
        let micros = i64::try_from(now.unix_nanos().div_euclid(1_000)).unwrap_or(i64::MAX);
        let since = temporal.minus(&Temporal::Timestamp.format(micros), window)?;
        Some((temporal, since))
    }

    /// The key columns, and the incremental one, of the source rows inside `window`.
    fn source_keys(
        &mut self,
        incremental: Option<&str>,
        window: Option<&(Temporal, String)>,
    ) -> Result<Vec<RecordBatch>, PortError> {
        let mut columns: Vec<String> = self.table.key.iter().map(|c| quote_identifier(c)).collect();
        if let Some(column) = incremental
            && !self.table.key.iter().any(|key| key == column)
        {
            columns.push(quote_identifier(column));
        }
        let from = format!(
            "{}.{}",
            quote_identifier(&self.attached.alias),
            quote_identifier(&self.table.name)
        );
        let mut sql = ["SELECT ", &columns.join(", "), " FROM ", &from].concat();
        let mut params = Vec::new();
        if let (Some(column), Some((_, since))) = (incremental, window) {
            sql.push_str(" WHERE ");
            sql.push_str(&quote_identifier(column));
            sql.push_str(" >= ?");
            params.push(SqlValue::Text(since.clone()));
        }
        self.attached.connection.query_arrow(&sql, &params)
    }

    /// The source rows missing from raw whose incremental value is older than `since`
    /// (MG-REQ-0420.1).
    fn late_rows(
        &self,
        source_rows: &[RecordBatch],
        keys: &Keys,
        raw_keys: &BTreeSet<OwnedRow>,
        column: &str,
        temporal: Option<Temporal>,
        since: String,
    ) -> Result<Option<LateRows>, PortError> {
        let failed = |error: ArrowError| PortError::Failed(error.to_string());
        let mut late = Vec::new();
        for batch in source_rows {
            let absent = keys.of(batch).map_err(failed)?;
            let older = older_than(batch, column, temporal, &since)?;
            let mask: BooleanArray = absent
                .iter()
                .zip(older)
                .map(|(key, older)| Some(older && !raw_keys.contains(key)))
                .collect();
            late.push(filter_record_batch(batch, &mask).map_err(failed)?);
        }
        let count: usize = late.iter().map(RecordBatch::num_rows).sum();
        if count == 0 {
            return Ok(None);
        }
        let oldest = oldest(&late, column)?.unwrap_or_default();
        Ok(Some(LateRows {
            source_name: self.source_name.clone(),
            table: self.table.name.clone(),
            count: u64::try_from(count).unwrap_or(u64::MAX),
            since,
            oldest,
        }))
    }
}

/// The incremental values of `batch` as microseconds since the epoch in UTC; NULL stays NULL.
fn micros(batch: &RecordBatch, column: &str) -> Result<Vec<Option<i64>>, PortError> {
    let failed = |error: ArrowError| PortError::Failed(error.to_string());
    let values = batch
        .column_by_name(column)
        .ok_or_else(|| PortError::Failed(format!("the rows have no column `{column}`")))?;
    let values = match values.data_type() {
        DataType::Timestamp(_, Some(_)) => {
            let utc = cast(
                values,
                &DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            )
            .map_err(failed)?;
            cast(&utc, &DataType::Int64).map_err(failed)?
        }
        _ => {
            let naive =
                cast(values, &DataType::Timestamp(TimeUnit::Microsecond, None)).map_err(failed)?;
            Arc::new(
                naive
                    .as_primitive::<TimestampMicrosecondType>()
                    .reinterpret_cast::<arrow::datatypes::Int64Type>(),
            ) as ArrayRef
        }
    };
    Ok(values
        .as_primitive::<arrow::datatypes::Int64Type>()
        .iter()
        .collect())
}

/// Whether each row of `batch` has an incremental value not older than `since`; NULL is
/// outside the window.
fn not_older(
    batch: &RecordBatch,
    column: &str,
    temporal: Temporal,
    since: &str,
) -> Result<Vec<bool>, PortError> {
    let since = temporal
        .parse(since)
        .ok_or_else(|| PortError::Failed(format!("`{since}` is not a value of `{column}`")))?;
    Ok(micros(batch, column)?
        .into_iter()
        .map(|value| value.is_some_and(|value| value >= since))
        .collect())
}

/// Whether each row of `batch` has an incremental value older than `since`.
fn older_than(
    batch: &RecordBatch,
    column: &str,
    temporal: Option<Temporal>,
    since: &str,
) -> Result<Vec<bool>, PortError> {
    if let Some(temporal) = temporal {
        return Ok(not_older(batch, column, temporal, since)?
            .into_iter()
            .zip(micros(batch, column)?)
            .map(|(inside, value)| value.is_some() && !inside)
            .collect());
    }
    let failed = |error: ArrowError| PortError::Failed(error.to_string());
    let values = batch
        .column_by_name(column)
        .ok_or_else(|| PortError::Failed(format!("the rows have no column `{column}`")))?;
    let bound = cast(&StringArray::from(vec![since]), values.data_type()).map_err(failed)?;
    let older = lt(values, &Scalar::new(bound)).map_err(failed)?;
    Ok(older.iter().map(|value| value == Some(true)).collect())
}

/// The text of the smallest non-NULL value of `column` in `batches`.
fn oldest(batches: &[RecordBatch], column: &str) -> Result<Option<String>, PortError> {
    let failed = |error: ArrowError| PortError::Failed(error.to_string());
    let mut best: Option<(OwnedRow, String)> = None;
    for batch in batches {
        let values = batch
            .column_by_name(column)
            .ok_or_else(|| PortError::Failed(format!("the rows have no column `{column}`")))?;
        let converter =
            RowConverter::new(vec![SortField::new(values.data_type().clone())]).map_err(failed)?;
        let rows = converter
            .convert_columns(std::slice::from_ref(values))
            .map_err(failed)?;
        for index in (0..values.len()).filter(|index| !values.is_null(*index)) {
            let row = rows.row(index);
            if best.as_ref().is_none_or(|(known, _)| row < known.row()) {
                best = Some((row.owned(), value_text(values.as_ref(), index)?));
            }
        }
    }
    Ok(best.map(|(_, text)| text))
}

/// The key columns of a table, and the comparable form of their values.
struct Keys {
    fields: Vec<(String, DataType)>,
    converter: RowConverter,
}

impl Keys {
    /// The key `columns` of rows of `schema`.
    fn new(schema: &Schema, columns: &[String]) -> Result<Self, ArrowError> {
        let fields = columns
            .iter()
            .map(|column| {
                let field = schema.field_with_name(column)?;
                Ok((column.clone(), field.data_type().clone()))
            })
            .collect::<Result<Vec<_>, ArrowError>>()?;
        let sort = fields
            .iter()
            .map(|(_, data_type)| SortField::new(data_type.clone()));
        Ok(Self {
            converter: RowConverter::new(sort.collect())?,
            fields,
        })
    }

    /// The key of each row of `batch`, its columns cast to the types of the key.
    fn of(&self, batch: &RecordBatch) -> Result<Vec<OwnedRow>, ArrowError> {
        let mut columns = Vec::new();
        for (name, data_type) in &self.fields {
            let column = batch.column_by_name(name).ok_or_else(|| {
                ArrowError::SchemaError(format!("the rows have no key column `{name}`"))
            })?;
            columns.push(cast(column, data_type)?);
        }
        let rows = self.converter.convert_columns(&columns)?;
        Ok(rows.iter().map(|row| row.owned()).collect())
    }

    /// The keys of every row of `batches`.
    fn set(&self, batches: &[RecordBatch]) -> Result<BTreeSet<OwnedRow>, ArrowError> {
        let mut keys = BTreeSet::new();
        for batch in batches {
            keys.extend(self.of(batch)?);
        }
        Ok(keys)
    }
}
