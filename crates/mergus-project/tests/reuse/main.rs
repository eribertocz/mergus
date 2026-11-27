//! Tests of macros (MG-REQ-0641, MG-REQ-0642, MG-REQ-0644) and seeds (MG-REQ-0645,
//! MG-REQ-0646). The sandboxed sessions are those of the real `Database` (see
//! `mg_req_0641_macros_read_from_the_catalog` in `mergus-ports`); the trees are written by
//! hand in the shape `json_serialize_sql` gives: only the nodes the walk reads.

mod macros;
mod seeds;

use std::path::Path;

use mergus_ports::{FileSystem, PortError};
use mergus_sim::SimFileSystem;

/// The root of every project of these tests.
const ROOT: &str = "/project";

/// A simulated project holding `files`, given relative to the root.
fn project(files: &[(&str, &[u8])]) -> Result<SimFileSystem, PortError> {
    let fs = SimFileSystem::new();
    for (path, contents) in files {
        let path = Path::new(ROOT).join(path);
        if let Some(parent) = path.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&path, contents)?;
    }
    Ok(fs)
}
