//! The run cycle: stages 1 to 4 (MG-REQ-0701 to MG-REQ-0703), stage 5 and its candidates
//! (MG-REQ-0704, MG-REQ-0705), the work zone across runs (MG-REQ-0503, MG-REQ-0707,
//! MG-REQ-0708), the finalizer (MG-REQ-0709) and the run limits (MG-REQ-0718), on the
//! simulated file system and the real DuckDB in memory.

mod execute;
mod finalizer;
mod fixture;
mod limits;
mod resume;
mod stages;
