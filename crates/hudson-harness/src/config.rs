use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// A projection of agent configuration. Contains no executor or credential authority.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub allow_user_input: bool,
    pub instructions: String,
    pub model: String,
    pub tools: Vec<ToolDescriptor>,
    pub max_context_bytes: usize,
}

impl Config {
    /// Descriptors allowed in model requests, including opt-in loop capabilities.
    pub fn model_tools(&self) -> Result<Vec<ToolDescriptor>, crate::HarnessError> {
        let mut tools = self.tools.clone();
        if self.allow_user_input {
            if tools.iter().any(|tool| tool.name == "ask_user") {
                return Err(crate::HarnessError::Invalid(
                    "ask_user is reserved when user input is enabled".into(),
                ));
            }
            tools.push(ToolDescriptor {
                name: "ask_user".into(),
                description: "Ask the user for missing information and wait for their reply. Call this alone, without other tools in the same response.".into(),
                input_schema: serde_json::json!({"type":"object","properties":{"prompt":{"type":"string","minLength":1,"maxLength":16384}},"required":["prompt"],"additionalProperties":false}),
            });
        }
        Ok(tools)
    }
}
