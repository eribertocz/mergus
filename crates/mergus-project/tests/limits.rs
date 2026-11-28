//! MG-REQ-0718.1: `[limits]` in `project.toml`.

use std::path::Path;
use std::time::Duration;

use mergus_ports::FileSystem;
use mergus_project::{LayoutError, ProjectLimits, read_configuration};
use mergus_sim::SimFileSystem;

const ROOT: &str = "/project";

fn read(project: &str) -> Result<mergus_project::Configuration, LayoutError> {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new(ROOT))?;
    files.write(&Path::new(ROOT).join("project.toml"), project.as_bytes())?;
    read_configuration(&files, Path::new(ROOT))
}

#[test]
fn mg_req_0718_limits_in_project_toml() {
    let valid = "name = \"sales\"\n[limits]\nwall_time = \"30m\"\nrows_written = 1000\n\
                 bytes_scanned = 5000000\n";
    let configuration = read(valid).unwrap();
    let limits = &configuration.project.value["limits"];
    assert_eq!(limits["wall_time"], "30m");
    assert_eq!(limits["rows_written"], 1000);
    assert_eq!(limits["bytes_scanned"], 5_000_000);
    assert_eq!(
        configuration.limits(),
        ProjectLimits {
            wall_time: Some(Duration::from_secs(1800)),
            rows_written: Some(1000),
            bytes_scanned: Some(5_000_000),
        }
    );
    let hours = read("name = \"sales\"\n[limits]\nwall_time = \"2h\"\n").unwrap();
    assert_eq!(hours.limits().wall_time, Some(Duration::from_secs(7200)));
    assert_eq!(hours.limits().rows_written, None);
    let none = read("name = \"sales\"\n").unwrap();
    assert_eq!(none.limits(), ProjectLimits::default());

    for invalid in [
        "wall_time = \"30d\"",
        "wall_time = \"0s\"",
        "wall_time = 30",
        "rows_written = 0",
        "bytes_scanned = \"10GB\"",
        "max_rows = 1",
    ] {
        let project = format!("name = \"sales\"\n[limits]\n{invalid}\n");
        let Err(LayoutError::Invalid(problems)) = read(&project) else {
            panic!("`{invalid}` was accepted");
        };
        assert_eq!(problems.len(), 1, "{invalid}: {problems:?}");
        assert!(
            problems
                .iter()
                .all(|problem| problem.path() == "project.toml"),
            "{problems:?}"
        );
    }
}
