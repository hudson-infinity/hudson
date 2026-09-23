use crate::{definitions, models::Assessment};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Deterministic criteria supplied by the customer, or held out by an evaluator.
/// JSON pointers address the candidate, so these checks do not execute model code.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Criterion {
    Equals {
        pointer: String,
        expected: Value,
    },
    NumberRange {
        pointer: String,
        min: f64,
        max: f64,
    },
    Contains {
        pointer: String,
        text: String,
    },
    /// Require the latest matching recorded tool execution to pass a customer check.
    ToolResultEquals {
        tool_name: String,
        #[serde(default)]
        require_latest_tool: bool,
        #[serde(default)]
        arguments: Option<Value>,
        pointer: String,
        expected: Value,
    },
}

impl Criterion {
    pub fn validate(&self) -> crate::Result<()> {
        let pointer = match self {
            Self::Equals { pointer, .. } | Self::Contains { pointer, .. } => pointer,
            Self::ToolResultEquals {
                tool_name, pointer, ..
            } => {
                if tool_name.trim().is_empty() || tool_name.len() > 256 {
                    return Err(crate::Error::Invalid(
                        "verification tool name is invalid".into(),
                    ));
                }
                pointer
            }
            Self::NumberRange { pointer, min, max } => {
                if !min.is_finite() || !max.is_finite() || min > max {
                    return Err(crate::Error::Invalid(
                        "invalid verification numeric range".into(),
                    ));
                }
                pointer
            }
        };
        if !pointer.is_empty() && !pointer.starts_with('/') {
            return Err(crate::Error::Invalid(
                "verification requires a JSON pointer".into(),
            ));
        }
        let bytes = pointer.as_bytes();
        for (i, byte) in bytes.iter().enumerate() {
            if *byte == b'~' && !matches!(bytes.get(i + 1), Some(b'0' | b'1')) {
                return Err(crate::Error::Invalid("invalid JSON pointer escape".into()));
            }
        }
        Ok(())
    }

    pub fn matches_with_operations(
        &self,
        candidate: &Value,
        operations: &[crate::models::Operation],
    ) -> bool {
        use crate::models::{OperationRequest, OperationResult, OperationStatus};
        let Self::ToolResultEquals {
            tool_name,
            require_latest_tool,
            arguments,
            pointer,
            expected,
        } = self
        else {
            return self.matches(candidate);
        };
        if self.validate().is_err() {
            return false;
        }
        let latest = operations.iter().filter(|operation| {
            matches!(&operation.request, OperationRequest::Tool { call, .. }
                if &call.name == tool_name && arguments.as_ref().is_none_or(|args| args == &call.arguments))
        }).max_by_key(|operation| (operation.step_index, operation.request_index));
        latest.is_some_and(|operation| {
            if *require_latest_tool && operations.iter().any(|other| {
                matches!(other.request, OperationRequest::Tool { .. }) &&
                (other.step_index, other.request_index) > (operation.step_index, operation.request_index)
            }) { return false; }
            operation.status == OperationStatus::Succeeded && matches!(&operation.result,
                Some(OperationResult::Tool { result }) if matches!(&result.outcome,
                    hudson_harness::ToolOutcome::Success { value } if value.pointer(pointer) == Some(expected)))
        })
    }

    pub fn matches(&self, candidate: &Value) -> bool {
        if self.validate().is_err() {
            return false;
        }
        match self {
            Self::ToolResultEquals { .. } => false,
            Self::Equals { pointer, expected } => candidate.pointer(pointer) == Some(expected),
            Self::NumberRange { pointer, min, max } => candidate
                .pointer(pointer)
                .and_then(Value::as_f64)
                .is_some_and(|v| v.is_finite() && v >= *min && v <= *max),
            Self::Contains { pointer, text } => candidate
                .pointer(pointer)
                .and_then(Value::as_str)
                .is_some_and(|v| v.contains(text)),
        }
    }
}

pub fn check(schema: Option<&Value>, candidate: &Value) -> Assessment {
    let passed = schema.is_none_or(|s| definitions::validate(s, candidate).is_ok());
    Assessment {
        evidence: vec![],
        passed,
        feedback: if passed {
            "output contract passed"
        } else {
            "output did not satisfy the configured schema"
        }
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn criteria_distinguish_wrong_answers_with_identical_shapes() {
        let rule = Criterion::Equals {
            pointer: "/total".into(),
            expected: json!(42),
        };
        assert!(rule.matches(&json!({"total":42})));
        assert!(!rule.matches(&json!({"total":41})));
        assert!(!rule.matches(&json!({"other":42})));
        let range = Criterion::NumberRange {
            pointer: "/total".into(),
            min: 40.,
            max: 42.,
        };
        assert!(range.matches(&json!({"total":42})));
        assert!(!range.matches(&json!({"total":"42"})));
    }

    #[test]
    fn invalid_rules_fail_closed_and_escaped_keys_work() {
        let invalid = Criterion::Equals {
            pointer: "/bad~2".into(),
            expected: Value::Null,
        };
        assert!(invalid.validate().is_err());
        assert!(!invalid.matches(&Value::Null));
        let escaped = Criterion::Equals {
            pointer: "/a~1b/~0".into(),
            expected: json!(true),
        };
        assert!(escaped.matches(&json!({"a/b":{"~":true}})));
    }
}
