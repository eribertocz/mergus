//! Deterministic simulator.
//!
//! Every port of `mergus-ports` has a simulated implementation here (MG-REQ-0131). Each one
//! is created with a seed and a fault plan; the same seed and the same calls give the same
//! results. Nothing here touches the outside world.
//!
//! A [`Simulator`] drives a [`Scenario`] from a seed: it replays a run step by step and
//! shrinks a failing run to a minimal case that still fails (MG-REQ-0162). Simulated time
//! advances without waiting (MG-REQ-0163).

#![forbid(unsafe_code)]

mod clock;
mod environment;
mod file_system;
mod network;
mod object_store;
mod process;
mod random;
mod rng;
mod simulation;
mod source;

pub use clock::{ClockFaults, SimClock};
pub use environment::SimEnvironment;
pub use file_system::SimFileSystem;
pub use network::{NetworkFaults, SimNetwork, TcpHandler};
pub use object_store::{ObjectStoreFaults, SimObjectStore};
pub use process::SimProcess;
pub use random::SimRandom;
pub use rng::Rate;
pub use simulation::{Failure, Outcome, Replay, Scenario, Simulator, StepRecord, Violation};
pub use source::{SimSourceConnector, SourceFaults};
