//! MG-REQ-0747: the `mergus_plan` contract 1.1.0 carries the execution fields, and its 1.0.0
//! documents stay valid (MG-REQ-0206).

use mergus_contracts::generated::mergus_plan_v1::{
    BoundaryTransport, GateStatus, MergusPlan, SegmentEngine, StepRefresh,
};
use mergus_contracts::{Schema, read};
use std::path::Path;

use mergus_ports::FileSystem;
use mergus_ports::real::RealFileSystem;
use serde_json::Value;

/// Reads a file of the `mergus_plan` v1 folder.
#[cfg(test)]
fn plan_file(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/mergus_plan/v1")
        .join(name);
    String::from_utf8(RealFileSystem.read(&path).unwrap()).unwrap()
}

/// Reads a JSON document of the `mergus_plan` v1 folder.
#[cfg(test)]
fn document(name: &str) -> Value {
    serde_json::from_str(&plan_file(name)).unwrap()
}

#[test]
fn mg_req_0747_plan_carries_execution_fields() {
    assert_eq!(plan_file("VERSION").trim(), "1.1.0");
    let schema = Schema::parse(&plan_file("schema.json")).unwrap();
    let plan = document("valid/execution_fields.json");
    assert!(schema.validate(&plan).is_empty());
    let plan: MergusPlan = read(&plan).unwrap();
    let step = &plan.steps[0];
    let segments = step.segments.as_ref().unwrap();
    assert_eq!(segments[1].engine, SegmentEngine::Polars);
    assert!(segments[1].polars_level.is_some());
    assert!(segments[0].polars_level.is_none());
    assert_eq!(
        step.boundaries.as_ref().unwrap()[1].transport,
        BoundaryTransport::Parquet
    );
    assert_eq!(plan.steps[1].refresh, StepRefresh::Period);
    assert_eq!(plan.steps[1].periods.as_ref().unwrap().len(), 2);
    let gates = plan.gates.as_ref().unwrap();
    assert_eq!(gates[1].status, GateStatus::NotApplicable);
    assert_eq!(gates[2].errors.as_ref().unwrap()[0].code, "MG-0401");
    for older in ["valid/two_steps.json", "valid/stale_upstream.json"] {
        let older = document(older);
        assert!(schema.validate(&older).is_empty());
        let older: MergusPlan = read(&older).unwrap();
        assert_eq!(older.gates, None);
        assert!(older.steps.iter().all(|step| step.segments.is_none()));
    }
}
