//! MG-REQ-0295: the `mergus_events` contract, and the fields each event type requires,
//! checked before an event is emitted.

#[cfg(test)]
mod tests {
    use mergus_contracts::generated::mergus_events_v1::{
        MergusEvents, MergusEventsSeverity, MergusEventsType,
    };
    use mergus_contracts::{Contract, EmitError, Schema, validate_event};
    use mergus_ports::FileSystem;
    use mergus_ports::real::RealFileSystem;
    use serde_json::Value;

    /// An event of `kind` with the common fields only.
    fn bare(kind: MergusEventsType) -> MergusEvents {
        MergusEvents {
            contract: MergusEvents::NAME.to_owned(),
            contract_version: 1,
            r#type: kind,
            run_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_owned(),
            sequence: 1,
            timestamp: "2026-11-22T23:15:14.123Z".to_owned(),
            actor: None,
            command: None,
            arguments: None,
            environment: None,
            steps: None,
            step_id: None,
            model: None,
            engine: None,
            status: None,
            rows: None,
            bytes: None,
            duration_ms: None,
            code: None,
            severity: None,
            message: None,
            location: None,
            hint: None,
            next: None,
            test: None,
            failing_rows: None,
            manifest_id: None,
            exit_code: None,
            retryable: None,
        }
    }

    fn missing(event: &MergusEvents) -> Vec<String> {
        match validate_event(event) {
            Ok(_) => Vec::new(),
            Err(EmitError::Invalid { violations, .. }) => {
                violations.into_iter().map(|v| v.message).collect()
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn mg_req_0295_events_schema_and_required_fields() {
        schema_is_one_closed_object();
        required_fields_are_checked();
        diagnostic_with_its_fields_is_valid();
    }

    fn schema_is_one_closed_object() {
        assert_eq!(MergusEvents::NAME, "mergus_events");
        assert_eq!(MergusEvents::VERSION, 1);
        assert_eq!(MergusEvents::SEMANTIC_VERSION, "1.0.0");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/mergus_events/v1/schema.json");
        let text = String::from_utf8(RealFileSystem.read(&path).unwrap()).unwrap();
        assert_eq!(
            text,
            MergusEvents::SCHEMA,
            "the schema lives in schemas/mergus_events/v1/"
        );
        Schema::parse(&text).unwrap();
        let schema: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            schema["additionalProperties"],
            Value::Bool(false),
            "one closed object"
        );
        assert_eq!(strings(&schema["properties"]["type"]["enum"]), TYPES);
        assert_eq!(
            strings(&schema["required"]),
            COMMON,
            "the fields a type requires are optional in the schema"
        );
    }

    const TYPES: [&str; 8] = [
        "run_started",
        "plan_ready",
        "step_started",
        "step_finished",
        "diagnostic",
        "quality_result",
        "published",
        "run_finished",
    ];

    const COMMON: [&str; 6] = [
        "contract",
        "contract_version",
        "type",
        "run_id",
        "sequence",
        "timestamp",
    ];

    fn strings(value: &Value) -> Vec<&str> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap())
            .collect()
    }

    fn required_fields_are_checked() {
        let cases: [(MergusEventsType, &[&str]); 8] = [
            (
                MergusEventsType::RunStarted,
                &["command", "arguments", "environment"],
            ),
            (MergusEventsType::PlanReady, &["steps"]),
            (
                MergusEventsType::StepStarted,
                &["step_id", "model", "engine"],
            ),
            (
                MergusEventsType::StepFinished,
                &["step_id", "status", "rows", "bytes", "duration_ms"],
            ),
            (
                MergusEventsType::Diagnostic,
                &["code", "severity", "message", "hint"],
            ),
            (
                MergusEventsType::QualityResult,
                &["test", "model", "status", "failing_rows"],
            ),
            (MergusEventsType::Published, &["environment", "manifest_id"]),
            (
                MergusEventsType::RunFinished,
                &["status", "exit_code", "retryable", "duration_ms"],
            ),
        ];
        for (kind, fields) in cases {
            let found = missing(&bare(kind));
            assert_eq!(found.len(), fields.len(), "{kind:?}: {found:?}");
            for field in fields {
                assert!(
                    found
                        .iter()
                        .any(|message| message.contains(&format!("`{field}`"))),
                    "{kind:?} requires `{field}`: {found:?}"
                );
            }
        }
    }

    fn diagnostic_with_its_fields_is_valid() {
        let mut diagnostic = bare(MergusEventsType::Diagnostic);
        diagnostic.code = Some("MG-0301".to_owned());
        diagnostic.severity = Some(MergusEventsSeverity::Error);
        diagnostic.message = Some("model `a` is defined twice".to_owned());
        diagnostic.hint = Some("Rename one of the files.".to_owned());
        let valid = validate_event(&diagnostic).unwrap();
        assert_eq!(valid.contract(), "mergus_events");
    }
}
