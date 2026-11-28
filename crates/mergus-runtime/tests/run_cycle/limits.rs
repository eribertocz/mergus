//! Run limits: wall time, rows written and bytes scanned stop the run before they are
//! exceeded, with nothing published, through the finalizer (MG-REQ-0718).

use std::time::Duration;

use mergus_ports::{FileSystem, Timestamp};
use mergus_runtime::{
    CANDIDATES, CancelToken, Limit, LimitStop, Limits, ModelWork, Run, RunError, RunReport,
    StepContext, StepFailure,
};
use mergus_sim::{ClockFaults, SimClock, SimFileSystem};

use crate::finalizer::only_stg_kept;
use crate::fixture::{
    COUNT, Log, at, cleared, database, fingerprint, models, orders, read, root, tree,
};

#[cfg(test)]
fn clock() -> SimClock {
    SimClock::new(7, Timestamp::from_unix_nanos(0), ClockFaults::default())
}

/// Runs the fixture one step at a time under `limits`, with `scanned` bytes read by the
/// plan; `slow` is a model whose work moves `clock` forward by 11 seconds. Returns the run's
/// report, the models started and the file system.
#[cfg(test)]
fn limited(
    limits: Limits,
    scanned: u64,
    slow: Option<&str>,
) -> (RunReport, Vec<String>, SimFileSystem, String) {
    let files = orders(COUNT);
    let clock = clock();
    let database = database();
    let project = read(&files);
    let cancel = CancelToken::new();
    let run = Run::begin_with_limits(&files, root(), "run1", cancel, limits, &clock).unwrap();
    let planned = crate::fixture::plan(&project, run.available(), 1);
    let adopted = run.adopt(&planned).unwrap();
    let log = Log::default();
    let mut work = models(&files, &database, &project, &log, None);
    if let Some(slow) = slow {
        let inner = work.models.remove(slow).unwrap();
        let slow_work = Slow {
            inner,
            clock: &clock,
        };
        work.models.insert(slow.to_owned(), Box::new(slow_work));
    }
    work.bytes_scanned = scanned;
    let executed = run.execute(&adopted, &cleared(), &mut work);
    drop(work);
    let report = run.finish(executed);
    let started = log
        .events()
        .into_iter()
        .filter_map(|event| event.strip_prefix("start ").map(str::to_owned))
        .collect();
    let stg = fingerprint(&planned, "stg_orders");
    (report, started, files, stg)
}

/// The limit stop `report` ended with.
#[cfg(test)]
fn stop(report: &RunReport) -> LimitStop {
    assert_eq!(report.error_code(), Some("MG-0705"));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    match &report.outcome {
        Err(RunError::Limit(stop)) => stop.clone(),
        other => panic!("not a limit stop: {other:?}"),
    }
}

#[test]
fn mg_req_0718_run_stops_at_limit_without_publishing() {
    // `stg_orders` writes 3 rows; the 2 rows of `orders_big` would make 5.
    let rows = Limits {
        rows_written: Some(4),
        ..Limits::default()
    };
    let (report, started, files, stg) = limited(rows, 0, None);
    let stopped = stop(&report);
    assert_eq!(
        stopped,
        LimitStop {
            limit: Limit::RowsWritten,
            value: 4,
            reached: 5,
        }
    );
    let message = RunError::Limit(stopped).to_string();
    assert!(message.contains("`rows_written`"), "{message}");
    assert!(message.contains('4') && message.contains('5'), "{message}");
    assert_eq!(started, ["stg_orders", "orders_big"]);
    // Only the candidate written before the stop is complete; nothing else is kept.
    only_stg_kept(&files, &stg);
    assert!(files.list_dir(&at(".mergus/work/runs")).unwrap().is_empty());
}

#[test]
fn mg_req_0718_rows_up_to_the_limit_are_written() {
    let rows = Limits {
        rows_written: Some(6),
        ..Limits::default()
    };
    let (report, started, _, _) = limited(rows, 0, None);
    assert_eq!(report.error_code(), None, "{:?}", report.outcome);
    assert_eq!(started, ["stg_orders", "orders_big", "orders_count"]);
}

#[test]
fn mg_req_0718_bytes_scanned_checked_before_stage_5() {
    let bytes = Limits {
        bytes_scanned: Some(100),
        ..Limits::default()
    };
    let (report, started, files, _) = limited(bytes, 101, None);
    let stopped = stop(&report);
    assert_eq!(
        stopped,
        LimitStop {
            limit: Limit::BytesScanned,
            value: 100,
            reached: 101,
        }
    );
    assert!(started.is_empty(), "{started:?}");
    assert!(tree(&files, &at(CANDIDATES)).is_empty());

    let (report, _, _, _) = limited(bytes, 100, None);
    assert_eq!(report.error_code(), None, "{:?}", report.outcome);
}

#[test]
fn mg_req_0718_wall_time_checked_before_each_segment_and_between_batches() {
    let wall = Limits {
        wall_time: Some(Duration::from_secs(10)),
        ..Limits::default()
    };
    // `orders_big` runs past the limit: its rows are not written, and no other segment
    // starts.
    let (report, started, files, stg) = limited(wall, 0, Some("orders_big"));
    let stopped = stop(&report);
    assert_eq!(
        stopped,
        LimitStop {
            limit: Limit::WallTime,
            value: 10_000,
            reached: 11_000,
        }
    );
    let message = RunError::Limit(stopped).to_string();
    assert!(message.contains("`wall_time`"), "{message}");
    assert_eq!(started, ["stg_orders", "orders_big"]);
    only_stg_kept(&files, &stg);
}

/// A work that moves the clock 11 seconds forward while it runs.
#[cfg(test)]
struct Slow<'a> {
    inner: Box<dyn ModelWork + 'a>,
    clock: &'a SimClock,
}

#[cfg(test)]
impl ModelWork for Slow<'_> {
    fn run(&self, context: &StepContext<'_>) -> Result<(), StepFailure> {
        let result = self.inner.run(context);
        self.clock.advance(Duration::from_secs(11));
        result
    }
}
