//! `cargo xtask simulation`: the invariants under `mergus-sim` (MG-REQ-0198).
//!
//! The scenarios are tests driven by `mergus_sim::Simulator`; this command chooses their
//! seeds through the environment and runs them:
//!
//! - `--seeds <n> [--first <seed>]`: `n` seeds from `first` (0 by default), as every pull
//!   request does with 1,000 (MG-REQ-0198.1);
//! - `--seed <seed> [--commit <sha>]`: replays one seed step by step, on the commit that
//!   failed (MG-REQ-0162);
//! - `--first <seed> --minutes <m>`: runs batches of new seeds from `first` until `m` minutes
//!   of simulation have passed, the build not counted, as each job of the release campaign
//!   does (MG-REQ-0198.2). It reports the next seed and the seconds it simulated.
//!
//! A failing seed fails the command; the scenario prints it with its minimal case and the
//! command that replays it (MG-REQ-0198.3).

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::failure::Failure;
use crate::{cargo, workspace};

/// The seeds of a pull request (MG-REQ-0198.1).
pub(crate) const PULL_REQUEST_SEEDS: u64 = 1_000;
/// The seeds of one batch of a campaign.
const BATCH: u64 = 2_000;
/// The scenarios, as `(package, test target, test)`.
const SCENARIOS: [(&str, &str, &str); 1] = [(
    "mergus-runtime",
    "incremental",
    "simulation::mg_req_0143_resume_is_exactly_once_under_the_simulator",
)];

/// Which seeds to run.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Seeds {
    /// `count` seeds from `first`.
    Range { first: u64, count: u64 },
    /// One seed, replayed on `commit` when given.
    Replay { seed: u64, commit: Option<String> },
    /// Batches of seeds from `first` for `minutes`.
    Campaign { first: u64, minutes: u64 },
}

/// Reads the arguments of `cargo xtask simulation`.
///
/// # Errors
///
/// Fails on an unknown flag, a missing or non-numeric value, or a mix of modes.
pub(crate) fn parse(args: &[String]) -> Result<Seeds, Failure> {
    let usage = || {
        Failure::new(
            "usage: cargo xtask simulation (--seeds <n> [--first <seed>] | --seed <seed> \
             [--commit <sha>] | --first <seed> --minutes <m>)",
        )
    };
    let mut values: [Option<&str>; 5] = [None; 5];
    let flags = ["--seeds", "--first", "--seed", "--commit", "--minutes"];
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let index = flags
            .iter()
            .position(|known| known == flag)
            .ok_or_else(usage)?;
        let value = rest.next().ok_or_else(usage)?;
        if values[index].replace(value).is_some() {
            return Err(usage());
        }
    }
    let number = |value: Option<&str>| -> Result<Option<u64>, Failure> {
        value
            .map(|text| text.parse().map_err(|_| usage()))
            .transpose()
    };
    let [seeds, first, seed, commit, minutes] = values;
    match (
        number(seeds)?,
        number(first)?,
        number(seed)?,
        commit,
        number(minutes)?,
    ) {
        (Some(count), first, None, None, None) if count > 0 => Ok(Seeds::Range {
            first: first.unwrap_or(0),
            count,
        }),
        (None, None, Some(seed), commit, None) => Ok(Seeds::Replay {
            seed,
            commit: commit.map(str::to_owned),
        }),
        (None, Some(first), None, None, Some(minutes)) if minutes > 0 => {
            Ok(Seeds::Campaign { first, minutes })
        }
        _ => Err(usage()),
    }
}

/// Runs `cargo xtask simulation`.
///
/// # Errors
///
/// Fails when the arguments are wrong, the commit to replay is not checked out, or a seed
/// breaks an invariant.
pub(crate) fn run(args: &[String]) -> Result<(), Failure> {
    let seeds = parse(args)?;
    let root = workspace::root()?;
    let head = head(&root)?;
    match seeds {
        Seeds::Range { first, count } => run_seeds(&root, &head, first, count),
        Seeds::Replay { seed, commit } => {
            if let Some(commit) = commit.filter(|commit| !head.starts_with(commit.as_str())) {
                return Err(Failure::new(format!(
                    "seed {seed} failed on commit {commit}, but {head} is checked out: run \
                     `git checkout {commit}` first"
                )));
            }
            run_seeds(&root, &head, seed, 1)
        }
        Seeds::Campaign { first, minutes } => campaign(&root, &head, first, minutes),
    }
}

/// Runs batches of seeds from `first` until `minutes` have passed, then reports them.
fn campaign(root: &Path, head: &str, first: u64, minutes: u64) -> Result<(), Failure> {
    let started = Instant::now();
    let budget = Duration::from_secs(minutes.saturating_mul(60));
    let mut next = first;
    while started.elapsed() < budget {
        run_seeds(root, head, next, BATCH)?;
        next = next
            .checked_add(BATCH)
            .ok_or_else(|| Failure::new("the seeds ran past the largest number"))?;
    }
    let seconds = started.elapsed().as_secs();
    report(&format!(
        "simulation: campaign ran seeds {first}..{next} in {seconds} seconds"
    ));
    report(&format!("next seed: {next}"));
    report(&format!("simulated seconds: {seconds}"));
    Ok(())
}

/// Runs `count` seeds from `first` of every scenario, at commit `head`.
fn run_seeds(root: &Path, head: &str, first: u64, count: u64) -> Result<(), Failure> {
    let last = first
        .checked_add(count)
        .ok_or_else(|| Failure::new("the seeds run past the largest number"))?;
    let (first_text, count_text) = (
        OsString::from(first.to_string()),
        OsString::from(count.to_string()),
    );
    let commit = OsString::from(head);
    let envs = [
        ("MERGUS_SIMULATION_FIRST_SEED", &first_text),
        ("MERGUS_SIMULATION_SEEDS", &count_text),
        ("MERGUS_SIMULATION_COMMIT", &commit),
    ];
    for (package, target, test) in SCENARIOS {
        let args = test_args(package, target, test, count == 1);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        cargo::run(root, &args, &envs).map_err(|_| {
            Failure::new(format!(
                "simulation: a seed of {first}..{last} broke an invariant at commit {head}; the \
                 failure above names it and the command that replays it"
            ))
        })?;
    }
    report(&format!(
        "simulation: seeds {first}..{last} passed at commit {head}"
    ));
    Ok(())
}

/// The arguments of `cargo` that run the scenario `test`; a replay shows its steps.
pub(crate) fn test_args(package: &str, target: &str, test: &str, replay: bool) -> Vec<String> {
    let mut args = [
        "test",
        "--locked",
        "--package",
        package,
        "--test",
        target,
        "--",
        test,
        "--exact",
    ]
    .map(str::to_owned)
    .to_vec();
    if replay {
        args.push("--nocapture".to_owned());
    }
    args
}

/// The commit checked out in `root`.
fn head(root: &Path) -> Result<String, Failure> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|error| Failure::new(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        return Err(Failure::new("`git rev-parse HEAD` failed"));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|_| Failure::new("`git rev-parse HEAD` printed non-UTF-8"))
}

/// Prints one line of the report on standard output.
#[expect(
    clippy::print_stdout,
    reason = "MG-REQ-0198: `cargo xtask simulation` reports the seeds it ran and the commit"
)]
fn report(line: &str) {
    println!("{line}");
}

#[cfg(test)]
mod tests {
    use super::{Seeds, parse, test_args};

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn mg_req_0198_seeds_of_a_pull_request() {
        assert_eq!(
            parse(&args("--seeds 1000")).unwrap(),
            Seeds::Range {
                first: 0,
                count: 1_000
            }
        );
        assert_eq!(
            parse(&args("--seeds 5 --first 7")).unwrap(),
            Seeds::Range { first: 7, count: 5 }
        );
    }

    #[test]
    fn mg_req_0198_replay_of_a_seed_on_its_commit() {
        assert_eq!(
            parse(&args("--seed 42 --commit abc123")).unwrap(),
            Seeds::Replay {
                seed: 42,
                commit: Some("abc123".to_owned())
            }
        );
        assert!(test_args("p", "t", "x", true).contains(&"--nocapture".to_owned()));
        assert!(!test_args("p", "t", "x", false).contains(&"--nocapture".to_owned()));
    }

    #[test]
    fn mg_req_0198_campaign_of_new_seeds() {
        assert_eq!(
            parse(&args("--first 99 --minutes 330")).unwrap(),
            Seeds::Campaign {
                first: 99,
                minutes: 330
            }
        );
    }

    #[test]
    fn mg_req_0198_wrong_arguments_are_refused() {
        for wrong in [
            "",
            "--seeds",
            "--seeds x",
            "--seeds 0",
            "--seeds 1 --seed 2",
            "--seed 1 --minutes 3",
            "--minutes 3",
            "--seeds 1 --seeds 2",
            "--other 1",
        ] {
            assert!(parse(&args(wrong)).is_err(), "accepted `{wrong}`");
        }
    }
}
