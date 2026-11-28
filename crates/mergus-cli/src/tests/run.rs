//! MG-REQ-0710: `mergus run` and `mergus backfill`.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_ports::real::RealDatabase;
use mergus_sim::{SimFileSystem, SimKeychain, SimRandom};

use super::{NoInput, Outcome, PROJECT, ROOT, Setup, exists, mergus, project};

const STG: &str = "SELECT 1 AS id UNION ALL SELECT 2 AS id UNION ALL SELECT 3 AS id";
const BIG: &str = "SELECT id FROM stg_orders WHERE id > 1";
const COUNT: &str = "SELECT count(*) AS n FROM stg_orders";

/// Three `.sql` models: `stg_orders`, then `orders_big` and `orders_count`, which read it.
pub(crate) fn orders() -> SimFileSystem {
    project(&[
        ("project.toml", PROJECT),
        ("models/stg_orders.sql", STG),
        ("models/orders_big.sql", BIG),
        ("models/orders_count.sql", COUNT),
    ])
}

/// Runs `mergus <args>` in [`ROOT`] of `files` on a real DuckDB in memory.
pub(crate) fn run_on_duckdb(files: &dyn FileSystem, args: &[&str]) -> Outcome {
    let database = RealDatabase::new(Path::new("/nonexistent"));
    let random = SimRandom::new(7);
    Setup {
        files,
        cwd: ROOT,
        args,
        vars: &[],
        random: &random,
        seed: 7,
        terminal: false,
        input: None,
    }
    .run_all(None, &SimKeychain::new(), &NoInput, Some(&database))
}

/// The complete candidates of `files`.
fn complete(files: &SimFileSystem) -> usize {
    let folder = Path::new(ROOT).join(".mergus/work/candidates");
    files.list_dir(&folder).map_or(0, |entries| {
        entries
            .iter()
            .filter(|entry| files.exists(&entry.join("complete.json")).unwrap())
            .count()
    })
}

#[test]
fn mg_req_0710_run_executes_into_candidates() {
    let files = orders();
    let outcome = run_on_duckdb(&files, &["run", "--json"]);
    assert_eq!(outcome.exit, 0, "{outcome:?}");

    // Stage 5 ran every step into a complete candidate; nothing is left in scratch.
    let ready = outcome.of("plan_ready");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0]["steps"].as_array().unwrap().len(), 3);
    assert_eq!(complete(&files), 3);
    let runs = Path::new(ROOT).join(".mergus/work/runs");
    assert!(files.list_dir(&runs).map_or(true, |runs| runs.is_empty()));
    let finished = outcome.of("run_finished");
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0]["exit_code"], 0);
    assert!(outcome.of("published").is_empty());
    assert!(
        outcome.stderr.contains("3 computed, 0 skipped"),
        "{outcome:?}"
    );
    assert!(
        outcome.stderr.contains("nothing was published"),
        "{outcome:?}"
    );

    // A second run reuses every candidate.
    let again = run_on_duckdb(&files, &["run"]);
    assert_eq!(again.exit, 0, "{again:?}");
    assert!(again.stdout.contains("0 computed, 3 skipped"), "{again:?}");
    assert!(again.stdout.contains("nothing was published"), "{again:?}");
}

#[test]
fn mg_req_0710_project_error_executes_nothing() {
    let files = project(&[
        ("project.toml", "name = \"sales\"\nnmae = \"x\"\n"),
        ("models/stg_orders.sql", STG),
    ]);
    let outcome = run_on_duckdb(&files, &["run", "--json"]);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert!(outcome.of("plan_ready").is_empty());
    assert!(!exists(&files, ".mergus/work"));
}

#[test]
fn mg_req_0710_failing_step_exits_with_its_code() {
    let files = project(&[
        ("project.toml", PROJECT),
        ("models/stg_orders.sql", STG),
        (
            "models/orders_bad.sql",
            "SELECT CAST('x' AS INTEGER) AS n FROM stg_orders",
        ),
    ]);
    let outcome = run_on_duckdb(&files, &["run", "--json"]);
    assert_ne!(outcome.exit, 0, "{outcome:?}");
    let finished = outcome.of("run_finished");
    assert_eq!(finished[0]["exit_code"], outcome.exit);
    assert_eq!(outcome.of("diagnostic").len(), 1, "{outcome:?}");
    // The step that succeeded keeps its candidate.
    assert_eq!(complete(&files), 1);
}

#[test]
fn mg_req_0710_backfill_needs_partition_by() {
    let files = orders();
    let outcome = run_on_duckdb(&files, &["backfill", "stg_orders", "--from", "2026-01-01"]);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert!(outcome.stderr.contains("MG-06"), "{outcome:?}");
    assert_eq!(complete(&files), 0);
}

#[test]
fn mg_req_0710_backfill_rejects_a_date_that_is_not_one() {
    let files = orders();
    let outcome = run_on_duckdb(&files, &["backfill", "stg_orders", "--from", "2026-13-01"]);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert!(outcome.stderr.contains("MG-0705"), "{outcome:?}");
}

#[test]
fn mg_req_0710_run_without_duckdb_executes_nothing() {
    let files = orders();
    let outcome = mergus(&files, &["run"]);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert!(outcome.stderr.contains("MG-0706"), "{outcome:?}");
    assert!(!exists(&files, ".mergus/work"));
}
