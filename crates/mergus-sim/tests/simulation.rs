//! MG-REQ-0162 and MG-REQ-0163: the simulator replays a seed exactly, shrinks a failure and
//! runs simulated time without waiting.

use std::sync::Arc;
use std::time::Duration;

use mergus_ports::{Clock, Random, Timestamp};
use mergus_sim::{ClockFaults, Outcome, Scenario, SimClock, Simulator, Violation};

const START: Timestamp = Timestamp::from_unix_nanos(1_800_000_000_000_000_000);

/// A store with a planted bug: it breaks once it has seen 13 and, later, 42.
struct PlantedBug;

impl Scenario for PlantedBug {
    type Step = u64;
    type State = bool;

    fn generate(&self, random: &dyn Random) -> Vec<u64> {
        (0..200).map(|_| random.next_u64() % 64).collect()
    }

    fn start(&self, _seed: u64) -> bool {
        false
    }

    fn apply(&self, seen_13: &mut bool, step: &u64) -> Result<(), Violation> {
        if *seen_13 && *step == 42 {
            return Err(Violation::new("42 after 13"));
        }
        *seen_13 |= *step == 13;
        Ok(())
    }
}

#[test]
fn mg_req_0162_seed_replays_and_shrinks() {
    let simulator = Simulator::new(PlantedBug);
    let failure = (0..1_000)
        .find_map(|seed| match simulator.run(seed) {
            Outcome::Failed(failure) => Some(failure),
            Outcome::Passed { .. } => None,
        })
        .expect("a seed below 1000 finds the planted bug");
    assert_eq!(failure.violation.message(), "42 after 13");
    assert_eq!(failure.steps.last(), Some(&42));

    // The seed alone replays the same failure.
    assert_eq!(
        simulator.run(failure.seed),
        Outcome::Failed(failure.clone())
    );
    assert_eq!(
        simulator.steps(failure.seed)[..failure.steps.len()],
        failure.steps[..]
    );

    // Step by step: every step passes until the one that failed, and the replay stops there.
    let records: Vec<_> = simulator
        .replay(failure.seed, simulator.steps(failure.seed))
        .collect();
    assert_eq!(records.len(), failure.steps.len());
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.index, index);
        assert_eq!(record.step, failure.steps[index]);
        assert_eq!(record.result.is_err(), index + 1 == records.len());
    }

    // Shrinking keeps a case that still fails and from which no step can be removed.
    let minimal = simulator.shrink(&failure);
    assert_eq!(minimal.steps, vec![13, 42]);
    assert_eq!(minimal.seed, failure.seed);
    assert_eq!(
        simulator.run_steps(minimal.seed, minimal.steps.clone()),
        Outcome::Failed(minimal.clone())
    );
}

/// A schedule that fires every hour and retries a failed attempt after ten minutes.
struct HourlySchedule;

impl Scenario for HourlySchedule {
    type Step = u64;
    type State = Arc<SimClock>;

    fn generate(&self, random: &dyn Random) -> Vec<u64> {
        (0..24 * 30).map(|_| random.next_u64() % 4).collect()
    }

    fn start(&self, seed: u64) -> Arc<SimClock> {
        Arc::new(SimClock::new(seed, START, ClockFaults::default()))
    }

    fn apply(&self, clock: &mut Arc<SimClock>, attempts: &u64) -> Result<(), Violation> {
        let before = clock.monotonic();
        for _ in 0..*attempts {
            clock.sleep(Duration::from_secs(10 * 60));
        }
        clock.sleep(Duration::from_secs(3_600));
        if clock.monotonic() <= before {
            return Err(Violation::new("time did not advance"));
        }
        Ok(())
    }
}

#[test]
fn mg_req_0163_simulated_time_is_accelerated() {
    #[allow(
        clippy::disallowed_methods,
        clippy::disallowed_types,
        reason = "the test measures real time to show that simulated time does not wait"
    )]
    let started = std::time::Instant::now();
    let simulator = Simulator::new(HourlySchedule);
    let mut clock = HourlySchedule.start(5);
    let steps = simulator.steps(5);
    for step in &steps {
        HourlySchedule
            .apply(&mut clock, step)
            .expect("time advances");
    }
    assert_eq!(simulator.run(5), Outcome::Passed { steps: steps.len() });

    // Thirty days of hourly runs and their retries took seconds, not days.
    assert!(clock.monotonic().elapsed() >= Duration::from_secs(30 * 24 * 3_600));
    assert!(started.elapsed() < Duration::from_secs(5));
}
