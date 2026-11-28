//! Stage 5 follows the plan, and leaves its results as candidates only in the work zone.

use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

use mergus_contracts::generated::mergus_plan_v1::{StepAction, StepReason};
use mergus_plan::Plan;
use mergus_ports::FileSystem;
use mergus_ports::real::RealDatabase;
use mergus_runtime::{
    CancelToken, Executed, ModelWork, ReadProject, Run, RunError, StepContext, StepFailure,
};
use mergus_sim::SimFileSystem;
use serde_json::Value;

use crate::fixture::{
    COUNT, Log, at, cleared, database, fingerprint, models, orders, plan, read, root, tree,
};

#[test]
fn mg_req_0704_execution_matches_plan() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    let planned = plan(&project, run.available(), 2);
    let adopted = run.adopt(&planned).unwrap();
    let log = Log::default();
    let mut work = models(&files, &database, &project, &log, None);
    let extraction_log = log.clone();
    work.extractions
        .push(Box::new(move |_cancel: &CancelToken| {
            extraction_log.push("extract".to_owned());
            Ok(())
        }));
    let executed = run.execute(&adopted, &cleared(), &mut work).unwrap();
    run.finish().unwrap();

    let Executed::Finished { rows } = executed else {
        panic!("not finished");
    };
    let expected: Vec<(String, u64)> = vec![
        ("orders_big".to_owned(), 2),
        ("orders_count".to_owned(), 1),
        ("stg_orders".to_owned(), 3),
    ];
    assert_eq!(rows.into_iter().collect::<Vec<_>>(), expected);
    let events = log.events();
    assert_eq!(
        events[..3],
        ["extract", "start stg_orders", "end stg_orders"]
    );
    let mut readers: Vec<&String> = events[3..]
        .iter()
        .filter(|e| e.starts_with("start"))
        .collect();
    readers.sort();
    assert_eq!(readers, ["start orders_big", "start orders_count"]);
    let most = log.most_at_once.load(Ordering::SeqCst);
    assert!(most <= 2, "{most} segments ran at once");

    one_at_a_time(&files, &database, &project, &planned);
}

/// A second plan with `max_parallel = 1` runs one segment at a time, and a step whose result
/// is available runs nothing.
#[cfg(test)]
fn one_at_a_time(
    files: &SimFileSystem,
    database: &RealDatabase,
    project: &ReadProject,
    planned: &Plan,
) {
    let run = Run::begin(files, root(), "run2", CancelToken::new()).unwrap();
    let mut available = run.available().clone();
    available.remove(&fingerprint(planned, "orders_big"));
    available.remove(&fingerprint(planned, "orders_count"));
    files
        .remove_dir_all(&at(".mergus/work/candidates").join(fingerprint(planned, "orders_big")))
        .unwrap();
    files
        .remove_dir_all(&at(".mergus/work/candidates").join(fingerprint(planned, "orders_count")))
        .unwrap();
    let planned = plan(project, &available, 1);
    let stg = &planned.document().steps[0];
    assert_eq!(
        (stg.action, stg.reason),
        (StepAction::Skip, StepReason::ResultAvailable)
    );
    let log = Log::default();
    let mut work = models(files, database, project, &log, None);
    let executed = run
        .execute(&run.adopt(&planned).unwrap(), &cleared(), &mut work)
        .unwrap();
    run.finish().unwrap();
    assert_eq!(executed.exit_code(), 0);
    assert_eq!(
        log.events(),
        [
            "start orders_big",
            "end orders_big",
            "start orders_count",
            "end orders_count"
        ]
    );
    assert_eq!(log.most_at_once.load(Ordering::SeqCst), 1);
}

#[test]
fn mg_req_0705_candidates_invisible() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    let before: BTreeSet<String> = tree(&files, root()).into_iter().collect();
    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    let planned = plan(&project, run.available(), 4);
    let log = Log::default();
    let mut work = models(&files, &database, &project, &log, None);
    // The completion of the first candidate crashes before its rename.
    files.crash_before_next_rename();
    let failed = run.execute(&run.adopt(&planned).unwrap(), &cleared(), &mut work);
    assert_eq!(failed.unwrap_err().code(), Some("MG-0704"));
    run.finish().unwrap();
    let stg = at(".mergus/work/candidates").join(fingerprint(&planned, "stg_orders"));
    assert!(
        !files.exists(&stg).unwrap(),
        "an incomplete candidate is kept"
    );

    let run = Run::begin(&files, root(), "run2", CancelToken::new()).unwrap();
    assert!(run.available().is_empty());
    let mut work = models(&files, &database, &project, &log, None);
    run.execute(&run.adopt(&planned).unwrap(), &cleared(), &mut work)
        .unwrap();
    run.finish().unwrap();

    let after: BTreeSet<String> = tree(&files, root()).into_iter().collect();
    for added in after.difference(&before) {
        assert!(
            added.starts_with(".mergus/work/candidates")
                || added == ".mergus"
                || added == ".mergus/work"
                || added == ".mergus/work/runs",
            "`{added}` is outside the candidates"
        );
    }
    assert_complete(&files, &planned);
}

/// Each candidate of `planned` holds its rows and its `complete.json`, written by `run2`.
#[cfg(test)]
fn assert_complete(files: &SimFileSystem, planned: &Plan) {
    for step in &planned.document().steps {
        let dir = at(".mergus/work/candidates").join(&step.fingerprint);
        let entries: Vec<String> = files
            .list_dir(&dir)
            .unwrap()
            .iter()
            .map(|entry| entry.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, ["complete.json", "result.arrow"]);
        let complete: Value =
            serde_json::from_slice(&files.read(&dir.join("complete.json")).unwrap()).unwrap();
        assert_eq!(complete["fingerprint"], step.fingerprint.as_str());
        assert_eq!(complete["run_id"], "run2");
        assert!(complete["rows"].as_u64().is_some());
    }
}

/// A work that fails with `code`, or panics when it has none.
#[cfg(test)]
struct Broken(Option<&'static str>);

#[cfg(test)]
impl ModelWork for Broken {
    fn run(&self, context: &StepContext<'_>) -> Result<(), StepFailure> {
        let Some(code) = self.0 else {
            panic!("the work of `{}` panics", context.model);
        };
        Err(StepFailure {
            name: context.model.to_owned(),
            code: Some(code),
            message: "broken".to_owned(),
        })
    }
}

#[test]
fn mg_req_0704_a_panicking_step_fails_the_run() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    let planned = plan(&project, run.available(), 4);
    let mut work = models(&files, &database, &project, &Log::default(), None);
    work.models
        .insert("orders_big".to_owned(), Box::new(Broken(None)));
    let adopted = run.adopt(&planned).unwrap();
    let failed = run.execute(&adopted, &cleared(), &mut work);
    assert_eq!(failed, Err(RunError::Panicked("orders_big".to_owned())));
    run.finish().unwrap();
}

#[test]
fn mg_req_0704_first_failure_in_plan_order() {
    for attempt in 0..5 {
        let files = orders(COUNT);
        let database = database();
        let project = read(&files);
        let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
        let planned = plan(&project, run.available(), 2);
        let mut work = models(&files, &database, &project, &Log::default(), None);
        work.models
            .insert("orders_big".to_owned(), Box::new(Broken(Some("MG-0101"))));
        work.models
            .insert("orders_count".to_owned(), Box::new(Broken(Some("MG-0102"))));
        let adopted = run.adopt(&planned).unwrap();
        let failed = run.execute(&adopted, &cleared(), &mut work).unwrap_err();
        assert_eq!(failed.code(), Some("MG-0101"), "attempt {attempt}");
        run.finish().unwrap();
    }
}
