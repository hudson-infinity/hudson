use super::VersionRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Execution {
    /// Explicitly registered trusted in-process executor; never arbitrary guest code.
    Registered {
        key: String,
    },
    Http {
        endpoint: String,
    },
    Sandbox {
        package: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Read,
    Write,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub id: String,
    pub workspace_id: String,
    pub version: u32,
    pub schema_version: u32,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub execution: Execution,
    pub credential_ref: Option<String>,
    pub policy_ref: String,
    pub effect: Effect,
    pub created_at: u64,
}

impl Tool {
    pub fn reference(&self) -> VersionRef {
        VersionRef {
            id: self.id.clone(),
            version: self.version,
        }
    }
}
