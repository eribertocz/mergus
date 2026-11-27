//! MG-REQ-0198 and MG-REQ-0143: INV-04 under `mergus-sim`.
//!
//! Each seed draws the loads of `sales`, read by chunks, and the commit after which the
//! process of each load crashes, if it does. After every load, the world that crashed and
//! resumed must show what an uninterrupted world shows: every row of `__changes`, finished or
//! not, the rows readers see, the current table and `ops.loads`.
//!
//! `cargo xtask simulation` chooses the seeds through the environment:
//! `MERGUS_SIMULATION_FIRST_SEED` and `MERGUS_SIMULATION_SEEDS` (by default 16 seeds from 0),
//! and `MERGUS_SIMULATION_COMMIT`, the commit named in the replay command of a failure.

use std::panic::{AssertUnwindSafe, catch_unwind};

use mergus_ports::Random;
use mergus_project::Source;
use mergus_sim::{Outcome, Scenario, Simulator, Violation};
use mergus_source::{DEFAULT_OVERLAP, DeclaredTable};
use serde_json::{Value, json};

use crate::ALL;
use crate::fixture::{Row, attach, attach_after, declared, source};
use crate::resume::{Snapshot, snapshot};
use crate::world::{World, texts, versions};

/// The seeds of a plain `cargo test`.
const DEFAULT_SEEDS: u64 = 16;
/// The keys a load can hold.
const KEYS: i64 = 8;

/// INV-04 over incremental loads by chunks, with a crash where the seed says.
struct ExactlyOnce;

/// One load of `sales`: the rows the source holds, the chunk size, and a number that
/// chooses after how many of the load's commits its process crashes; `None` never crashes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Load {
    rows: Vec<Row>,
    chunk: u32,
    crash: Option<u64>,
}

/// The same loads applied to two worlds: one never crashes, the other crashes and resumes.
struct Worlds {
    options: Value,
    source: Source,
    table: DeclaredTable,
    whole: World,
    crashed: World,
    /// The `updated_at` hours of the rows earlier loads read, which give the watermarks.
    earlier: Vec<i64>,
}

#[cfg(test)]
impl Scenario for ExactlyOnce {
    type Step = Load;
    type State = Worlds;

    fn generate(&self, random: &dyn Random) -> Vec<Load> {
        let loads = 1 + random.next_u64() % 4;
        (0..loads).map(|index| draw(random, index)).collect()
    }

    fn start(&self, seed: u64) -> Worlds {
        // The delete column, or a `compare` at every run.
        let options = if seed.is_multiple_of(2) {
            json!({ "deletes": "compare", "delete_check": "every_run" })
        } else {
            json!({ "deletes": "column:removed" })
        };
        let source = source(options.clone());
        let table = declared(&source);
        Worlds {
            options,
            source,
            table,
            whole: World::seeded(seed, seed.wrapping_add(1)),
            crashed: World::seeded(seed, seed.wrapping_add(1)),
            earlier: Vec::new(),
        }
    }

    fn apply(&self, worlds: &mut Worlds, load: &Load) -> Result<(), Violation> {
        catch_unwind(AssertUnwindSafe(|| apply(worlds, load))).unwrap_or_else(|panic| {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                .unwrap_or_default();
            Err(Violation::new(format!("panicked: {message}")))
        })
    }
}

#[cfg(test)]
/// Draws the load `index`: each key is present three times in four, an `updated_at` near
/// `10:00 + 2h × index` or NULL, and removed one time in eight.
fn draw(random: &dyn Random, index: u64) -> Load {
    let base = 9 + 2 * i64::try_from(index).unwrap();
    let rows = (1..=KEYS)
        .filter_map(|id| {
            let draw = random.next_u64();
            (!draw.is_multiple_of(4)).then(|| Row {
                id,
                amount: id * 10 + i64::try_from(draw / 4 % 3).unwrap(),
                hour: (!(draw / 12).is_multiple_of(10))
                    .then(|| base + i64::try_from(draw / 120 % 3).unwrap()),
                removed: (draw / 360).is_multiple_of(8),
            })
        })
        .collect();
    let chunk = 1 + u32::try_from(random.next_u64() % 3).unwrap();
    let crash = random.next_u64();
    Load {
        rows,
        chunk,
        crash: (!crash.is_multiple_of(4)).then_some(crash / 4),
    }
}

#[cfg(test)]
/// Runs `load` on both worlds and compares what they show.
fn apply(worlds: &mut Worlds, load: &Load) -> Result<(), Violation> {
    let Worlds {
        options,
        source,
        table,
        whole,
        crashed,
        earlier,
    } = worlds;
    let source_of = || attached(load, earlier);
    let before = commits(whole);
    let mut attached = source_of();
    whole
        .load(&mut attached, source, table, Some(load.chunk), None)
        .map_err(|error| Violation::new(format!("the uninterrupted load failed: {error}")))?;
    let made = commits(whole) - before;
    if let Some(crash) = load.crash {
        let after = usize::try_from(crash % u64::try_from(made).unwrap()).unwrap();
        let mut attached = source_of();
        let crashed_load =
            crashed.load(&mut attached, source, table, Some(load.chunk), Some(after));
        if crashed_load.is_ok() {
            return Err(Violation::new(format!(
                "the load did not crash after {after} of its {made} commits"
            )));
        }
    }
    let mut attached = source_of();
    crashed
        .load(&mut attached, source, table, Some(load.chunk), None)
        .map_err(|error| Violation::new(format!("the resumed load failed: {error}")))?;
    earlier.extend(load.rows.iter().filter_map(|row| row.hour));
    let (expected, seen) = (shown(whole), shown(crashed));
    if seen == expected {
        Ok(())
    } else {
        Err(Violation::new(format!(
            "{options}: after a crash the world shows {seen:?}, an uninterrupted one {expected:?}"
        )))
    }
}

#[cfg(test)]
/// The source holding the rows of `load`, which also answers reads from the watermarks of
/// the `earlier` hours.
fn attached(load: &Load, earlier: &[i64]) -> mergus_source::AttachedSource {
    let chunk = Some(load.chunk);
    attach_after(
        &load.rows,
        earlier,
        chunk,
        DEFAULT_OVERLAP,
        None,
        &ALL,
        Vec::new(),
    )
}

/// [`snapshot`], `(sale_id, amount, _deleted)` of every row of `__changes`, and each load's
/// `result`.
type Shown = (Snapshot, Vec<(i64, i64, bool)>, Vec<Option<String>>);

#[cfg(test)]
/// What `world` shows: [`snapshot`], every row of `__changes`, finished or not, and the
/// `result` of each row of `ops.loads`.
fn shown(world: &World) -> Shown {
    let results = texts(&world.registry(), "result");
    (snapshot(world), versions(&world.every_change()), results)
}

#[cfg(test)]
/// The commits of raw and ops together.
fn commits(world: &World) -> usize {
    world.raw.commits() + world.ops.commits()
}

#[cfg(test)]
/// Reads the environment variable `name` as a number, or `default` when it is not set.
#[expect(
    clippy::disallowed_methods,
    reason = "MG-REQ-0198: `cargo xtask simulation` chooses the seeds of the test through the \
              environment, the only input a test binary takes besides its filters"
)]
fn number(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |value| {
        value
            .parse()
            .unwrap_or_else(|_| panic!("{name} is not a number: {value}"))
    })
}

#[cfg(test)]
/// The commit named in the replay command, when `cargo xtask simulation` gives it.
#[expect(
    clippy::disallowed_methods,
    reason = "MG-REQ-0198: `cargo xtask simulation` names the commit under test through the \
              environment"
)]
fn commit() -> Option<String> {
    std::env::var("MERGUS_SIMULATION_COMMIT").ok()
}

#[cfg(test)]
/// The command that replays `seed` (MG-REQ-0162).
fn replay_command(seed: u64) -> String {
    let commit = commit().map_or_else(String::new, |sha| format!(" --commit {sha}"));
    format!("cargo xtask simulation --seed {seed}{commit}")
}

#[test]
fn mg_req_0143_resume_is_exactly_once_under_the_simulator() {
    let first = number("MERGUS_SIMULATION_FIRST_SEED", 0);
    let seeds = number("MERGUS_SIMULATION_SEEDS", DEFAULT_SEEDS);
    let simulator = Simulator::new(ExactlyOnce);
    if seeds == 1 {
        // A replay shows every step and its result.
        for record in simulator.replay(first, simulator.steps(first)) {
            println!(
                "step {}: {:?} -> {:?}",
                record.index, record.step, record.result
            );
        }
    }
    for seed in first..first.checked_add(seeds).unwrap() {
        if let Outcome::Failed(failure) = simulator.run(seed) {
            let minimal = simulator.shrink(&failure);
            panic!(
                "seed {seed} breaks INV-04 at step {}: {}\nminimal case: {:?}\nreplay: {}",
                failure.steps.len() - 1,
                failure.violation,
                minimal.steps,
                replay_command(seed)
            );
        }
    }
    println!("simulation: seeds {first}..{} passed", first + seeds);
}

#[test]
fn mg_req_0198_failure_names_its_replay_command() {
    let command = replay_command(42);
    assert!(command.starts_with("cargo xtask simulation --seed 42"));
}

#[test]
fn mg_req_0198_every_load_crashes_where_its_seed_says() {
    // Seeds draw loads with and without a crash, by chunks of one row or more, with removed
    // rows and NULL incremental values.
    let simulator = Simulator::new(ExactlyOnce);
    let steps: Vec<Load> = (0..200).flat_map(|seed| simulator.steps(seed)).collect();
    assert!(steps.iter().any(|load| load.crash.is_none()));
    assert!(steps.iter().any(|load| load.crash.is_some()));
    assert!(steps.iter().any(|load| load.chunk == 1));
    assert!(
        steps
            .iter()
            .any(|load| load.rows.iter().any(|row| row.removed))
    );
    assert!(
        steps
            .iter()
            .any(|load| load.rows.iter().any(|row| row.hour.is_none()))
    );
    // The same seed draws the same loads.
    assert_eq!(simulator.steps(7), simulator.steps(7));
}

#[test]
fn mg_req_0198_a_duplicated_load_is_a_violation() {
    // A world that ran one load twice differs from the uninterrupted world.
    let simulator = Simulator::new(ExactlyOnce);
    let load = simulator.steps(0).remove(0);
    let mut worlds = ExactlyOnce.start(0);
    let mut attached = attach(&load.rows, Some(load.chunk), DEFAULT_OVERLAP, None, &ALL);
    let (source, table) = (worlds.source.clone(), worlds.table.clone());
    worlds
        .crashed
        .load(&mut attached, &source, &table, Some(load.chunk), None)
        .unwrap();
    let violation = ExactlyOnce.apply(&mut worlds, &load).unwrap_err();
    assert!(
        violation
            .message()
            .contains("after a crash the world shows")
    );
}
