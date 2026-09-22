use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionRef {
    pub id: String,
    pub version: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Actor {
    pub workspace_id: String,
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub max_harness_steps: u64,
    pub max_model_calls: u32,
    pub max_operations: u32,
    pub max_batch_size: usize,
    pub max_context_bytes: usize,
    pub max_payload_bytes: usize,
}
impl Limits {
    pub fn validate(&self) -> crate::Result<()> {
        if self.max_harness_steps == 0
            || self.max_model_calls == 0
            || self.max_operations == 0
            || self.max_batch_size == 0
            || self.max_context_bytes == 0
            || self.max_payload_bytes == 0
        {
            return Err(crate::Error::Invalid(
                "all execution limits must be positive".into(),
            ));
        }
        Ok(())
    }
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_harness_steps: 128,
            max_model_calls: 8,
            max_operations: 32,
            max_batch_size: 4,
            max_context_bytes: 64 * 1024,
            max_payload_bytes: 64 * 1024,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    /// Admitted calls, not provider token/currency estimates.
    pub model_calls: u32,
    pub tool_calls: u32,
    pub operations: u32,
    /// Only calls with a valid provider usage report contribute to these totals.
    #[serde(default)]
    pub reported_model_calls: u32,
    #[serde(default)]
    pub reported_input_tokens: u64,
    #[serde(default)]
    pub reported_output_tokens: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    /// Total reported input, including cached input when the provider separates it.
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    pub id: Uuid,
    pub workspace_id: String,
    pub schema_version: u32,
    pub created_at: u64,
}
impl Metadata {
    pub fn new(workspace: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            workspace_id: workspace.into(),
            schema_version: SCHEMA_VERSION,
            created_at: now(),
        }
    }
}

/// Unix milliseconds (UTC). Runtime code injects explicit times for approval checks.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
