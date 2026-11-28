//! Run limits on wall time, rows written and bytes scanned (MG-REQ-0718).

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use mergus_ports::{Clock, MonotonicTime};

/// The limits of a run: the smaller of `[limits]` in `project.toml` and the `--max-*` flags,
/// which the caller resolves (MG-REQ-0718.1). `None` sets no limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    /// The longest a run may take from its start.
    pub wall_time: Option<Duration>,
    /// The most rows a run may write into its candidates.
    pub rows_written: Option<u64>,
    /// The most bytes of DuckLake files the plan may read.
    pub bytes_scanned: Option<u64>,
}

/// One limit of [`Limits`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    /// `wall_time`, in milliseconds.
    WallTime,
    /// `rows_written`, in rows.
    RowsWritten,
    /// `bytes_scanned`, in bytes.
    BytesScanned,
}

impl Limit {
    /// The key of the limit in `[limits]`.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::WallTime => "wall_time",
            Self::RowsWritten => "rows_written",
            Self::BytesScanned => "bytes_scanned",
        }
    }

    /// The unit its value and amount are counted in.
    const fn unit(self) -> &'static str {
        match self {
            Self::WallTime => "ms",
            Self::RowsWritten => "rows",
            Self::BytesScanned => "bytes",
        }
    }
}

/// A run stopped by a limit before it was exceeded (MG-REQ-0718.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LimitStop {
    /// The limit.
    pub limit: Limit,
    /// Its value.
    pub value: u64,
    /// The amount reached: the wall time elapsed, the rows the next batch would have made,
    /// or the bytes the plan reads.
    pub reached: u64,
}

impl fmt::Display for LimitStop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = self.limit.unit();
        write!(
            formatter,
            "the run stopped at its limit `{}` of {} {unit}: it reached {} {unit}",
            self.limit.key(),
            self.value,
            self.reached
        )
    }
}

/// What a run has used of its limits, shared by the threads of stage 5.
pub(crate) struct Meter<'a> {
    limits: Limits,
    clock: Option<(&'a dyn Clock, MonotonicTime)>,
    rows: AtomicU64,
}

impl<'a> Meter<'a> {
    /// A meter of `limits`, with wall time measured on `clock` from now.
    pub(crate) fn new(limits: Limits, clock: Option<&'a dyn Clock>) -> Self {
        Self {
            limits,
            clock: clock.map(|clock| (clock, clock.monotonic())),
            rows: AtomicU64::new(0),
        }
    }

    /// Fails when the plan reads more than `bytes_scanned`.
    pub(crate) fn check_bytes(&self, scanned: u64) -> Result<(), LimitStop> {
        match self.limits.bytes_scanned {
            Some(value) if scanned > value => Err(LimitStop {
                limit: Limit::BytesScanned,
                value,
                reached: scanned,
            }),
            _ => Ok(()),
        }
    }

    /// Fails when the run has taken longer than `wall_time`.
    pub(crate) fn check_time(&self) -> Result<(), LimitStop> {
        let (Some(limit), Some((clock, start))) = (self.limits.wall_time, self.clock) else {
            return Ok(());
        };
        let elapsed = clock.monotonic().elapsed().saturating_sub(start.elapsed());
        if elapsed > limit {
            return Err(LimitStop {
                limit: Limit::WallTime,
                value: millis(limit),
                reached: millis(elapsed),
            });
        }
        Ok(())
    }

    /// Counts the `rows` of a batch about to be written; fails, counting nothing, when they
    /// would cross `rows_written`.
    pub(crate) fn write_rows(&self, rows: u64) -> Result<(), LimitStop> {
        let Some(value) = self.limits.rows_written else {
            self.rows.fetch_add(rows, Ordering::SeqCst);
            return Ok(());
        };
        self.rows
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |written| {
                let total = written.saturating_add(rows);
                (total <= value).then_some(total)
            })
            .map(|_| ())
            .map_err(|written| LimitStop {
                limit: Limit::RowsWritten,
                value,
                reached: written.saturating_add(rows),
            })
    }
}

/// `duration` in whole milliseconds.
fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
