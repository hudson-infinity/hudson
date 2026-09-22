use crate::{definitions, models::Assessment};
use serde_json::Value;

/// First slice: deterministic final-output schema checking only.
pub fn check(schema: Option<&Value>, candidate: &Value) -> Assessment {
    let passed = schema.is_none_or(|s| definitions::validate(s, candidate).is_ok());
    Assessment {
        passed,
        feedback: if passed {
            "output contract passed"
        } else {
            "output did not satisfy the configured schema"
        }
        .into(),
    }
}
