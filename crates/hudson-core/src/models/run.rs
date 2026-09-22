use super::{Limits, Metadata, Usage, VersionRef};
use hudson_harness::{Checkpoint, Input};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Waiting,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WaitReason {
    Approval {
        operation_id: Uuid,
    },
    Reconciliation {
        operation_id: Uuid,
    },
    UserInput {
        prompt: String,
        #[serde(default)]
        question_id: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Assessment {
    pub passed: bool,
    pub feedback: String,
}

/// Immutable task objective and deterministic final-output acceptance contract.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Goal {
    pub objective: String,
    pub success_schema: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Run {
    pub meta: Metadata,
    pub agent_ref: VersionRef,
    #[serde(default)]
    pub parent_operation: Option<Uuid>,
    #[serde(default)]
    pub model_budget: Option<crate::budgets::ModelBudgetBinding>,
    pub actor_id: String,
    pub request_key: Option<String>,
    pub input_digest: String,
    pub input: Value,
    #[serde(default)]
    pub goal: Option<Goal>,
    pub status: RunStatus,
    pub reason: Option<String>,
    pub wait: Option<WaitReason>,
    pub state: Checkpoint,
    pub pending_operations: Vec<Uuid>,
    pub next_input: Option<Input>,
    pub limits: Limits,
    pub usage: Usage,
    pub result: Option<Value>,
    pub assessment: Option<Assessment>,
    pub verified_candidate_digest: Option<String>,
    pub revision: u64,
}

/// Deliberately excludes checkpoint, credentials, raw requests, and worker internals.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunView {
    pub agent_ref: VersionRef,
    pub parent_operation: Option<Uuid>,
    pub goal: Option<Goal>,
    pub id: Uuid,
    pub status: RunStatus,
    pub wait: Option<WaitReason>,
    pub reason: Option<String>,
    pub result: Option<Value>,
    pub usage: Usage,
    pub assessment: Option<Assessment>,
}
impl From<&Run> for RunView {
    fn from(run: &Run) -> Self {
        Self {
            agent_ref: run.agent_ref.clone(),
            id: run.meta.id,
            parent_operation: run.parent_operation,
            goal: run.goal.clone(),
            status: run.status,
            wait: run.wait.clone(),
            reason: run.reason.clone(),
            result: run.result.clone(),
            usage: run.usage.clone(),
            assessment: run.assessment.clone(),
        }
    }
}
