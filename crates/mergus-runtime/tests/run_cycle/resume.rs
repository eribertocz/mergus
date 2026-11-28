//! The work zone across runs: scratch deleted, finished work reused, cancellation cleaned.

use mergus_contracts::generated::mergus_plan_v1::{StepAction, StepReason};
use mergus_ports::FileSystem;
use mergus_runtime::{
    CANCELLED_EXIT_CODE, CancelToken, Executed, ModelWork, Run, StepContext, StepFailure,
};

use crate::fixture::{
    COUNT, Log, cleared, database, fingerprint, models, orders, plan, read, root, tree,
};

const CANDIDATES: &str = ".mergus/work/candidates";

#[test]
fn mg_req_0503_work_zone_cleaned() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    files
        .create_dir_all(&root().join(".mergus/work/runs/crashed/steps"))
        .unwrap();
    files
        .write(&root().join(".mergus/work/runs/crashed/steps/spill"), b"x")
        .unwrap();
    files
        .write(&root().join(".mergus/work/runs/stray"), b"x")
        .unwrap();

    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    assert_eq!(
        tree(&files, &root().join(".mergus/work/runs")),
        [".mergus/work/runs/run1"]
    );
    let planned = plan(&project, run.available(), 4);
    let mut work = models(&files, &database, &project, &Log::default(), None);
    run.execute(&planned, &cleared(), &mut work).unwrap();
    assert!(!tree(&files, run.scratch()).is_empty());
    run.finish().unwrap();
    assert!(tree(&files, &root().join(".mergus/work/runs")).is_empty());
    assert_eq!(tree(&files, &root().join(CANDIDATES)).len(), 9);

    // A failed run deletes its scratch directory too.
    struct Failing;
    impl ModelWork for Failing {
        fn run(&self, _context: &StepContext<'_>) -> Result<(), StepFailure> {
            Err(StepFailure {
                name: "orders_big".to_owned(),
                code: Some("MG-0101"),
                message: "no".to_owned(),
            })
        }
    }
    let files = orders(COUNT);
    let run = Run::begin(&files, root(), "run2", CancelToken::new()).unwrap();
    let mut work = models(&files, &database, &project, &Log::default(), None);
    work.models
        .insert("orders_big".to_owned(), Box::new(Failing));
    let failed = run.execute(&planned, &cleared(), &mut work).unwrap_err();
    assert_eq!(failed.code(), Some("MG-0101"));
    run.finish().unwrap();
    assert!(tree(&files, &root().join(".mergus/work/runs")).is_empty());
}

#[test]
fn mg_req_0707_retry_reuses_finished_work() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    let first = plan(&project, run.available(), 4);
    let mut work = models(&files, &database, &project, &Log::default(), None);
    run.execute(&first, &cleared(), &mut work).unwrap();
    run.finish().unwrap();
    // An incomplete candidate is deleted at the start of the next run.
    files
        .create_dir_all(&root().join(CANDIDATES).join("half"))
        .unwrap();
    files
        .write(&root().join(CANDIDATES).join("half/result.arrow"), b"x")
        .unwrap();

    // The retried run reuses every result: nothing runs.
    let run = Run::begin(&files, root(), "run2", CancelToken::new()).unwrap();
    assert!(!files.exists(&root().join(CANDIDATES).join("half")).unwrap());
    assert_eq!(run.available().len(), 3);
    let retried = plan(&project, run.available(), 4);
    for step in &retried.document().steps {
        assert_eq!(
            (step.action, step.reason),
            (StepAction::Skip, StepReason::ResultAvailable)
        );
    }
    let log = Log::default();
    let mut work = models(&files, &database, &project, &log, None);
    run.execute(&retried, &cleared(), &mut work).unwrap();
    run.finish().unwrap();
    assert!(log.events().is_empty());

    // A changed model is computed alone, from the candidate of its input; the candidate of its
    // old fingerprint, which no step names, is deleted.
    let files_changed = orders("SELECT count(*) AS n FROM stg_orders WHERE id > 0");
    for path in tree(&files, &root().join(CANDIDATES)) {
        let path = root().join(path);
        if files.list_dir(&path).is_ok() {
            files_changed.create_dir_all(&path).unwrap();
        } else {
            files_changed
                .write(&path, &files.read(&path).unwrap())
                .unwrap();
        }
    }
    let changed = read(&files_changed);
    let run = Run::begin(&files_changed, root(), "run3", CancelToken::new()).unwrap();
    let third = plan(&changed, run.available(), 4);
    run.adopt(&third).unwrap();
    let old = root()
        .join(CANDIDATES)
        .join(fingerprint(&first, "orders_count"));
    assert!(!files_changed.exists(&old).unwrap());
    let log = Log::default();
    let mut work = models(&files_changed, &database, &changed, &log, None);
    let executed = run.execute(&third, &cleared(), &mut work).unwrap();
    run.finish().unwrap();
    assert_eq!(log.events(), ["start orders_count", "end orders_count"]);
    let Executed::Finished { rows } = executed else {
        panic!("not finished");
    };
    assert_eq!(rows.get("orders_count"), Some(&1));
}

#[test]
fn mg_req_0708_cancel_cleans_and_exits_7() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    let cancel = CancelToken::new();
    let run = Run::begin(&files, root(), "run1", cancel.clone()).unwrap();
    let planned = plan(&project, run.available(), 1);
    let log = Log::default();
    // `orders_big` sets the token while it runs: its rows are not copied, `orders_count`
    // never starts.
    let mut work = models(&files, &database, &project, &log, Some("orders_big"));
    let executed = run.execute(&planned, &cleared(), &mut work).unwrap();
    assert!(cancel.is_cancelled());
    assert_eq!(executed, Executed::Cancelled);
    assert_eq!(executed.exit_code(), CANCELLED_EXIT_CODE);
    assert_eq!(executed.exit_code(), 7);
    assert_eq!(
        log.events(),
        [
            "start stg_orders",
            "end stg_orders",
            "start orders_big",
            "end orders_big"
        ]
    );
    let scratch = run.scratch().to_path_buf();
    run.finish().unwrap();
    assert!(!files.exists(&scratch).unwrap());
    let kept = tree(&files, &root().join(CANDIDATES));
    let stg = format!("{CANDIDATES}/{}", fingerprint(&planned, "stg_orders"));
    assert_eq!(
        kept,
        [
            stg.clone(),
            format!("{stg}/complete.json"),
            format!("{stg}/result.arrow")
        ]
    );

    // A token set before stage 5 starts no segment.
    let cancel = CancelToken::new();
    cancel.cancel();
    let run = Run::begin(&files, root(), "run2", cancel).unwrap();
    let log = Log::default();
    let mut work = models(&files, &database, &project, &log, None);
    let executed = run.execute(&planned, &cleared(), &mut work).unwrap();
    run.finish().unwrap();
    assert_eq!(executed, Executed::Cancelled);
    assert!(log.events().is_empty());
}
