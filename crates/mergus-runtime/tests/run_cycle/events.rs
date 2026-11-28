//! Stage 5 reports each computed step as it starts and ends, and nothing of a `skip` step
//! (MG-REQ-0710.2).

use mergus_contracts::generated::mergus_plan_v1::{Step, StepAction};
use mergus_ports::FileSystem;
use mergus_runtime::{CancelToken, Run, StepEnd, StepEvents};

use crate::fixture::{COUNT, Log, at, cleared, database, end, models, orders, plan, read, root};

/// Every event, as `started <model> <fingerprint>` or `finished <model> <rows> <bytes>`.
#[derive(Default)]
struct Recorded(Vec<String>, Vec<(String, u64)>);

impl StepEvents for Recorded {
    fn started(&mut self, step: &Step) {
        self.0
            .push(format!("started {} {}", step.model, step.fingerprint));
    }

    fn finished(&mut self, step: &Step, end: &StepEnd) {
        self.0.push(format!(
            "finished {} {} {}",
            step.model, end.succeeded, end.rows
        ));
        self.1.push((step.fingerprint.clone(), end.bytes));
    }
}

#[test]
fn mg_req_0710_step_events_of_computed_steps_only() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    let planned = plan(&project, run.available(), 1);
    let log = Log::default();
    let mut work = models(&files, &database, &project, &log, None);
    let mut recorded = Recorded::default();
    let adopted = run.adopt(&planned).unwrap();
    run.execute_with_events(&adopted, &cleared(), &mut work, &mut recorded)
        .unwrap();
    end(run);

    let steps = &planned.document().steps;
    let mut expected = Vec::new();
    for step in steps {
        expected.push(format!("started {} {}", step.model, step.fingerprint));
        expected.push(format!("finished {} true {}", step.model, rows(step)));
    }
    assert_eq!(recorded.0, expected);
    for (fingerprint, bytes) in &recorded.1 {
        let result = at(".mergus/work/candidates")
            .join(fingerprint)
            .join(mergus_runtime::RESULT);
        let size = u64::try_from(files.read(&result).unwrap().len()).unwrap();
        assert_eq!(*bytes, size, "{fingerprint}");
    }

    // A second run skips every step: no step events at all.
    let run = Run::begin(&files, root(), "run2", CancelToken::new()).unwrap();
    let planned = plan(&project, run.available(), 1);
    assert!(
        planned
            .document()
            .steps
            .iter()
            .all(|step| step.action == StepAction::Skip)
    );
    let mut work = models(&files, &database, &project, &log, None);
    let mut recorded = Recorded::default();
    let adopted = run.adopt(&planned).unwrap();
    run.execute_with_events(&adopted, &cleared(), &mut work, &mut recorded)
        .unwrap();
    end(run);
    assert!(recorded.0.is_empty(), "{:?}", recorded.0);
}

/// The rows the fixture's `orders` project writes for `step`.
fn rows(step: &Step) -> u64 {
    match step.model.as_str() {
        "stg_orders" => 3,
        "orders_big" => 2,
        _ => 1,
    }
}
