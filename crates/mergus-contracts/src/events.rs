//! The fields each event type requires besides the common ones (MG-REQ-0295).
//!
//! The `mergus_events` schema is one closed object for every type, so the fields a type
//! requires are optional there; [`validate_event`] checks them before the event is emitted.

use serde_json::Value;

use crate::Contract;
use crate::emit::{EmitError, ValidDocument, validate};
use crate::generated::mergus_events_v1::{MergusEvents, MergusEventsType};
use crate::validate::Violation;

/// Returns the fields an event of `kind` requires besides the common ones
/// (`contracts/events.md` section 1). A diagnostic's `location` is absent when the cause has
/// no location (MG-REQ-0296.4), so it is not listed.
#[must_use]
pub fn required_fields(kind: MergusEventsType) -> &'static [&'static str] {
    match kind {
        MergusEventsType::RunStarted => &["command", "arguments", "environment"],
        MergusEventsType::PlanReady => &["steps"],
        MergusEventsType::StepStarted => &["step_id", "model", "engine"],
        MergusEventsType::StepFinished => &["step_id", "status", "rows", "bytes", "duration_ms"],
        MergusEventsType::Diagnostic => &["code", "severity", "message", "hint"],
        MergusEventsType::QualityResult => &["test", "model", "status", "failing_rows"],
        MergusEventsType::Published => &["environment", "manifest_id"],
        MergusEventsType::RunFinished => &["status", "exit_code", "retryable", "duration_ms"],
    }
}

/// Validates `event` against the `mergus_events` schema, then checks that it carries every
/// field its type requires.
///
/// # Errors
///
/// Fails as [`validate`] does, and with [`EmitError::Invalid`] listing every required field
/// of its type the event lacks.
pub fn validate_event(event: &MergusEvents) -> Result<ValidDocument, EmitError> {
    let valid = validate(event)?;
    let violations: Vec<Violation> = required_fields(event.r#type)
        .iter()
        .filter(|field| matches!(valid.value().get(**field), None | Some(Value::Null)))
        .map(|field| Violation {
            pointer: String::new(),
            message: format!("the property `{field}`, required by the event type, is missing"),
        })
        .collect();
    if violations.is_empty() {
        Ok(valid)
    } else {
        Err(EmitError::Invalid {
            contract: MergusEvents::NAME,
            violations,
        })
    }
}
