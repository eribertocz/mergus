//! The automatic brake: the latency of each chunk is measured, the pace halved when it
//! exceeds twice the running median and restored when it recovers, and the extraction of the
//! source stopped after three consecutive failures (MG-REQ-0428).

use std::time::Duration;

use arrow::record_batch::RecordBatch;
use mergus_ports::Clock;

use crate::care::Budget;
use crate::engine::AttachedSource;
use crate::read::{ReadOptions, read_table};
use crate::table::DeclaredTable;

/// The consecutive failures that stop the extraction of a source.
const FAILURES_TO_STOP: u32 = 3;

/// An extra pause shorter than this is dropped, so that halving it ends.
const SMALLEST_EXTRA: Duration = Duration::from_millis(1);

/// The pace of the reads of one source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brake {
    /// The budget's pause.
    base: Duration,
    /// The pause before the next chunk.
    pause: Duration,
    /// The latency of every chunk read, sorted.
    latencies: Vec<Duration>,
    /// The failures since the last chunk read.
    failures: u32,
}

/// What the brake decides after a chunk statement failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterFailure {
    /// Send the chunk statement again.
    Retry,
    /// Stop the extraction of the source (MG-REQ-0428).
    Stop,
}

impl Brake {
    /// A brake at the pace of `budget`.
    #[must_use]
    pub fn new(budget: &Budget) -> Self {
        Self {
            base: budget.pause,
            pause: budget.pause,
            latencies: Vec::new(),
            failures: 0,
        }
    }

    /// The pause before the next chunk.
    #[must_use]
    pub const fn pause(&self) -> Duration {
        self.pause
    }

    /// Records a chunk read in `latency`. Above twice the running median of the chunks
    /// before it, the pause doubles, starting from `latency` when it is zero
    /// (MG-REQ-0428.1); at most the median, the pause above the budget's is halved
    /// (MG-REQ-0428.2).
    pub fn chunk_read(&mut self, latency: Duration) {
        self.failures = 0;
        if let Some(median) = self.median() {
            if latency > median.saturating_mul(2) {
                self.pause = if self.pause.is_zero() {
                    latency
                } else {
                    self.pause.saturating_mul(2)
                };
            } else if latency <= median {
                let extra = self.pause.saturating_sub(self.base) / 2;
                self.pause = if extra < SMALLEST_EXTRA {
                    self.base
                } else {
                    self.base + extra
                };
            }
        }
        let at = self.latencies.partition_point(|known| *known <= latency);
        self.latencies.insert(at, latency);
    }

    /// Records a chunk statement that failed or timed out (MG-REQ-0428.3), and says whether
    /// to retry it or to stop.
    pub fn chunk_failed(&mut self) -> AfterFailure {
        self.failures += 1;
        if self.failures >= FAILURES_TO_STOP {
            AfterFailure::Stop
        } else {
            AfterFailure::Retry
        }
    }

    /// The running median of the latencies, the lower one of the two middle ones.
    fn median(&self) -> Option<Duration> {
        let middle = self.latencies.len().checked_sub(1)? / 2;
        self.latencies.get(middle).copied()
    }
}

/// The extraction of a source stopped after three consecutive failures (MG-REQ-0428.4). It
/// is not retried until the next trigger.
#[derive(Debug, thiserror::Error)]
#[error(
    "the extraction of source `{source_name}` stopped after {failures} consecutive failures \
     reading table `{table}`; it is not retried until the next trigger. Last failure: {last}"
)]
pub struct SourceIncident {
    /// The source name.
    pub source_name: String,
    /// The table whose chunk failed.
    pub table: String,
    /// The consecutive failures.
    pub failures: u32,
    /// The message of the last failure.
    pub last: String,
}

impl SourceIncident {
    /// The `MG-####` code of the incident (MG-REQ-0008).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "MG-0405"
    }
}

/// Reads `table` of the source `source_name` from `attached` chunk after chunk, waiting on
/// `clock` for the brake's pause between two chunks and measuring each chunk's latency on
/// its monotonic time. A failed chunk is sent again, until three consecutive failures stop
/// the extraction (MG-REQ-0428).
///
/// # Errors
///
/// Fails with a [`SourceIncident`] after three consecutive failures.
pub fn read_with_brake(
    attached: &mut AttachedSource,
    source_name: &str,
    table: &DeclaredTable,
    options: &ReadOptions,
    brake: &mut Brake,
    clock: &dyn Clock,
) -> Result<Vec<RecordBatch>, SourceIncident> {
    let mut batches = Vec::new();
    let mut reader = read_table(attached, table, options);
    let mut first = true;
    while !reader.is_done() {
        if !first && !brake.pause().is_zero() {
            clock.sleep(brake.pause());
        }
        first = false;
        let started = clock.monotonic();
        let Some(chunk) = reader.next() else {
            break;
        };
        let latency = clock
            .monotonic()
            .elapsed()
            .saturating_sub(started.elapsed());
        match chunk {
            Ok(chunk) => {
                brake.chunk_read(latency);
                batches.extend(chunk);
            }
            Err(last) => {
                if brake.chunk_failed() == AfterFailure::Stop {
                    return Err(SourceIncident {
                        source_name: source_name.to_owned(),
                        table: table.name.clone(),
                        failures: FAILURES_TO_STOP,
                        last: last.to_string(),
                    });
                }
                reader.retry();
            }
        }
    }
    Ok(batches)
}
