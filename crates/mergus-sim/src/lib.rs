//! Deterministic simulator.
//!
//! Every port of `mergus-ports` has a simulated implementation here (MG-REQ-0131). Each one
//! is created with a seed and a fault plan; the same seed and the same calls give the same
//! results. Nothing here touches the outside world.

#![forbid(unsafe_code)]

mod clock;
mod environment;
mod file_system;
mod network;
mod object_store;
mod process;
mod random;
mod rng;
mod source;

pub use clock::{ClockFaults, SimClock};
pub use environment::SimEnvironment;
pub use file_system::SimFileSystem;
pub use network::{NetworkFaults, SimNetwork, TcpHandler};
pub use object_store::{ObjectStoreFaults, SimObjectStore};
pub use process::SimProcess;
pub use random::SimRandom;
pub use rng::Rate;
pub use source::{SimSourceConnector, SourceFaults};
