use super::{Metadata, VersionRef};
use hudson_harness::{ModelRequest, ModelResponse, ToolCall, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationRequest {
    Model {
        request: ModelRequest,
    },
    Tool {
        tool_ref: VersionRef,
        call: ToolCall,
    },
    Verify {
        candidate: Value,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationResult {
    Model { response: ModelResponse },
    Tool { result: ToolResult },
    Verify { passed: bool, feedback: String },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Pending,
    WaitingApproval,
    Running,
    Unknown,
    Succeeded,
    Failed,
    Denied,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ApprovalDecision {
    pub actor_id: String,
    pub request_digest: String,
    pub approved: bool,
    pub decided_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Approval {
    pub request_digest: String,
    pub decisions: Vec<ApprovalDecision>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Attempt {
    #[serde(default)]
    pub token_usage: Option<super::TokenUsage>,
    pub id: Uuid,
    pub number: u32,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub status: OperationStatus,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Operation {
    pub meta: Metadata,
    pub run_id: Uuid,
    pub step_index: u64,
    pub request_index: usize,
    pub request: OperationRequest,
    pub request_digest: String,
    pub status: OperationStatus,
    pub approval: Option<Approval>,
    pub attempts: Vec<Attempt>,
    pub result: Option<OperationResult>,
    pub revision: u64,
}

/// An approval-review projection, not an executable request or credential record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationView {
    #[serde(default)]
    pub token_usage: Option<super::TokenUsage>,
    pub id: Uuid,
    pub run_id: Uuid,
    pub kind: String,
    pub status: OperationStatus,
    pub tool_name: Option<String>,
    pub arguments: Option<Value>,
    pub request_digest: String,
    pub approval: Option<Approval>,
}
impl From<&Operation> for OperationView {
    fn from(operation: &Operation) -> Self {
        let (kind, tool_name, arguments) = match &operation.request {
            OperationRequest::Model { .. } => ("model", None, None),
            OperationRequest::Verify { .. } => ("verify", None, None),
            OperationRequest::Tool { call, .. } => (
                "tool",
                Some(call.name.clone()),
                Some(call.arguments.clone()),
            ),
        };
        Self {
            token_usage: operation
                .attempts
                .last()
                .and_then(|attempt| attempt.token_usage.clone()),
            id: operation.meta.id,
            run_id: operation.run_id,
            kind: kind.into(),
            status: operation.status,
            tool_name,
            arguments,
            request_digest: operation.request_digest.clone(),
            approval: operation.approval.clone(),
        }
    }
}
