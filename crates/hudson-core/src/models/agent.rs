use super::{Limits, VersionRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AgentTool {
    pub tool_ref: VersionRef,
    pub alias: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Agent {
    pub id: String,
    pub workspace_id: String,
    pub version: u32,
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub allow_user_input: bool,
    pub instructions: String,
    pub model: String,
    pub tools: Vec<AgentTool>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub limits: Limits,
    pub created_at: u64,
}

impl Agent {
    pub fn reference(&self) -> VersionRef {
        VersionRef {
            id: self.id.clone(),
            version: self.version,
        }
    }
}
