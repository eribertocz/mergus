//! MG-REQ-0617 and MG-REQ-0636.1: what the engine does with a child's report, and the names
//! that cannot leave the run's work directory. No real interpreter runs here.

use std::path::{Path, PathBuf};

use mergus_engine_polars::{Parameter, PolarsError, Step, execute_step, plan_step, step_dir};
use mergus_ports::real::RealFileSystem;
use mergus_ports::{FileSystem, PortError, ProcessOutput, Python, PythonTree};
use mergus_sim::SimPython;

use super::{floats, scratch};

/// A child that leaves `files` in its working directory and exits with `code`.
struct FakeChild {
    files: Vec<(&'static str, Vec<u8>)>,
    code: i32,
}

impl Python for FakeChild {
    fn parse(&self, source: &str) -> Result<PythonTree, PortError> {
        SimPython::new().parse(source)
    }

    fn run(&self, _: &str, _: &[String], dir: &Path) -> Result<ProcessOutput, PortError> {
        for (name, bytes) in &self.files {
            RealFileSystem.write(&dir.join(name), bytes)?;
        }
        Ok(ProcessOutput {
            code: Some(self.code),
            stdout: Vec::new(),
            stderr: b"Segmentation fault".to_vec(),
        })
    }
}

#[cfg(test)]
fn named(model: &str, step: &str) -> Step {
    Step {
        model: model.to_owned(),
        step: step.to_owned(),
        module: PathBuf::from("/project/models/model.py"),
        lib: PathBuf::from("/project/lib"),
    }
}

#[cfg(test)]
fn parameters() -> Vec<Parameter> {
    vec![Parameter {
        name: "joined".to_owned(),
        schema: floats(&["amount"]),
    }]
}

#[test]
fn mg_req_0617_child_without_a_valid_report_is_a_child_error() {
    let work = scratch("contract-report");
    let step = named("sales", "in_usd");
    let missing = FakeChild {
        files: Vec::new(),
        code: -11,
    };
    let error = plan_step(&missing, &RealFileSystem, &step, &parameters(), &work).unwrap_err();
    assert!(
        matches!(&error, PolarsError::Child { reason, .. } if reason.contains("Segmentation fault")),
        "{error:?}"
    );
    assert_eq!(error.code(), None);
    let corrupt = FakeChild {
        files: vec![("status.json", b"{\"status\": ".to_vec())],
        code: 0,
    };
    let error = plan_step(&corrupt, &RealFileSystem, &step, &parameters(), &work).unwrap_err();
    assert!(matches!(&error, PolarsError::Child { .. }), "{error:?}");
}

#[test]
fn mg_req_0617_truncated_output_is_a_boundary_error() {
    let work = scratch("contract-output");
    let step = named("sales", "in_usd");
    let truncated = FakeChild {
        files: vec![
            ("status.json", b"{\"status\": \"ok\"}".to_vec()),
            ("output.arrow", b"ARROW1\0\0truncated".to_vec()),
        ],
        code: 0,
    };
    let error = execute_step(&truncated, &RealFileSystem, &step, &[], &work).unwrap_err();
    assert!(matches!(&error, PolarsError::Boundary { .. }), "{error:?}");
    assert_eq!(error.code(), None);
}

#[test]
fn mg_req_0636_names_never_leave_the_work_directory() {
    let work = scratch("contract-names");
    let outside = work.join("outside");
    RealFileSystem.create_dir_all(&outside).unwrap();
    let absolute = outside.display().to_string();
    for (model, step) in [
        ("..", "x"),
        (".", "x"),
        ("", "x"),
        ("a/b", "x"),
        ("sales", ".."),
        ("sales", absolute.as_str()),
        ("sales", "a\\b"),
        ("C:", "x"),
    ] {
        if cfg!(not(windows)) && (model == "C:" || step == "a\\b") {
            // `\` and `C:` are plain characters of a name outside Windows.
            continue;
        }
        let step = named(model, step);
        assert!(
            matches!(step_dir(&work, &step), Err(PolarsError::InvalidName { .. })),
            "{model:?} / {:?}",
            step.step
        );
        let error = plan_step(
            &FakeChild {
                files: Vec::new(),
                code: 0,
            },
            &RealFileSystem,
            &step,
            &parameters(),
            &work,
        );
        assert!(matches!(error, Err(PolarsError::InvalidName { .. })));
    }
    // Nothing outside the step's directory was removed.
    assert!(RealFileSystem.exists(&outside).unwrap());
    let good = step_dir(&work, &named("sales", "in_usd")).unwrap();
    assert_eq!(good, work.join("polars").join("sales").join("in_usd"));
}
