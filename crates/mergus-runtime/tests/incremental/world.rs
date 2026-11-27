//! The stores of one world, raw and ops, which outlive the processes that load them.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use arrow::array::{AsArray, RecordBatch};
use arrow::compute::{concat_batches, take_record_batch};
use arrow::datatypes::{DataType, Int64Type};
use mergus_adapter::VersionRef;
use mergus_ports::Timestamp;
use mergus_project::Source;
use mergus_runtime::{
    LoadError, LoadOutcome, Stores, TableLoad, TableLoads, finished_changes, load_table,
    table_loads,
};
use mergus_sim::{ClockFaults, SimClock, SimRandom};
use mergus_source::{AttachedSource, DeclaredTable};

use crate::fixture::{DAY_ONE, HOUR};
use crate::memory::Memory;

/// The stores of raw and ops, which outlive a process, and the clock of the world.
pub struct World {
    pub raw: Memory,
    pub ops: Memory,
    pub clock: SimClock,
    pub random: SimRandom,
}

#[cfg(test)]
impl World {
    pub fn new() -> Self {
        Self::seeded(1, 7)
    }

    /// A world whose clock and random numbers take the seeds `clock` and `random`.
    pub fn seeded(clock: u64, random: u64) -> Self {
        Self {
            raw: Memory::default(),
            ops: Memory::default(),
            clock: SimClock::new(
                clock,
                Timestamp::from_unix_nanos(i128::from(DAY_ONE + 12 * HOUR) * 1_000),
                ClockFaults::default(),
            ),
            random: SimRandom::new(random),
        }
    }

    pub fn raw_version(&self) -> Option<VersionRef> {
        version(self.raw.commits())
    }

    pub fn ops_version(&self) -> Option<VersionRef> {
        version(self.ops.commits())
    }

    /// Loads `table` of `source` from `attached` in a process that crashes after `crash`
    /// commits, raw and ops together; `None` never crashes.
    pub fn load(
        &mut self,
        attached: &mut AttachedSource,
        source: &Source,
        table: &DeclaredTable,
        chunk: Option<u32>,
        crash: Option<usize>,
    ) -> Result<LoadOutcome, LoadError> {
        let left = crash.map(|left| Arc::new(AtomicUsize::new(left)));
        self.raw.crash = left.clone();
        self.ops.crash = left;
        let load = TableLoad {
            source,
            table,
            chunk_rows: chunk.and_then(NonZeroU32::new),
            raw_version: self.raw_version(),
            ops_version: self.ops_version(),
        };
        let mut stores = Stores {
            raw: &mut self.raw,
            ops: &mut self.ops,
            clock: &self.clock,
            random: &self.random,
        };
        let outcome = load_table(&mut stores, attached, &load);
        self.raw.crash = None;
        self.ops.crash = None;
        self.clock.advance(Duration::from_secs(60));
        outcome
    }

    /// The rows of `__changes` readers see, sorted by key, then `_deleted`, then amount.
    pub fn changes(&self, table: &DeclaredTable) -> RecordBatch {
        let rows = finished_changes(
            &self.raw,
            &self.ops,
            "erp",
            table,
            self.raw_version().as_ref(),
            self.ops_version().as_ref(),
        )
        .unwrap()
        .unwrap();
        sorted(&rows)
    }

    /// Every row of `__changes`, finished or not, sorted.
    pub fn every_change(&self) -> RecordBatch {
        sorted(&self.raw.latest("erp__sales__changes"))
    }

    /// The current rows, sorted.
    pub fn current(&self) -> RecordBatch {
        sorted(&self.raw.latest("erp__sales"))
    }

    /// The loads of `sales` recorded.
    pub fn loads(&self) -> TableLoads {
        table_loads(&self.ops, &self.ops_version().unwrap(), "erp", "sales").unwrap()
    }

    /// Every row of `ops.loads`.
    pub fn registry(&self) -> RecordBatch {
        self.ops.latest("loads")
    }
}

#[cfg(test)]
fn version(commits: usize) -> Option<VersionRef> {
    (commits > 0).then(|| VersionRef::new(commits.to_string()))
}

#[cfg(test)]
/// `rows` sorted by `sale_id`, `_deleted` and `amount`.
pub fn sorted(rows: &RecordBatch) -> RecordBatch {
    let rows = concat_batches(&rows.schema(), [rows]).unwrap();
    let mut order: Vec<usize> = (0..rows.num_rows()).collect();
    let ids = ints(&rows, "sale_id");
    let amounts = ints(&rows, "amount");
    let deleted = flags(&rows, "_deleted");
    order.sort_by_key(|&row| (ids[row], deleted[row], amounts[row]));
    let indices = arrow::array::UInt32Array::from_iter_values(order.iter().map(|&row| row as u32));
    take_record_batch(&rows, &indices).unwrap()
}

#[cfg(test)]
pub fn ints(batch: &RecordBatch, column: &str) -> Vec<i64> {
    let column = batch.column_by_name(column).unwrap();
    column.as_primitive::<Int64Type>().values().to_vec()
}

#[cfg(test)]
pub fn flags(batch: &RecordBatch, column: &str) -> Vec<bool> {
    let column = batch.column_by_name(column).unwrap().as_boolean();
    column.iter().map(|value| value.unwrap()).collect()
}

#[cfg(test)]
pub fn texts(batch: &RecordBatch, column: &str) -> Vec<Option<String>> {
    let mut column = batch.column_by_name(column).unwrap().clone();
    if let DataType::Timestamp(..) = column.data_type() {
        column = arrow::compute::cast(&column, &DataType::Int64).unwrap();
    }
    let column = arrow::compute::cast(&column, &DataType::Utf8).unwrap();
    column
        .as_string::<i32>()
        .iter()
        .map(|value| value.map(str::to_owned))
        .collect()
}

#[cfg(test)]
/// `(sale_id, amount, _deleted)` of each row of `batch`.
pub fn versions(batch: &RecordBatch) -> Vec<(i64, i64, bool)> {
    let ids = ints(batch, "sale_id");
    let amounts = ints(batch, "amount");
    let deleted = flags(batch, "_deleted");
    (0..batch.num_rows())
        .map(|row| (ids[row], amounts[row], deleted[row]))
        .collect()
}
