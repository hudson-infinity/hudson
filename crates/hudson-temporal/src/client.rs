//! Customer-facing execution client. Submit/pin a run through the core runtime
//! first, then start it here; the worker owns all model/tool orchestration.
use crate::{RunWorkflow, RunWorkflowDefinition};
use hudson_core::models::RunView;
use temporalio_client::{
    Client, WorkflowGetResultOptions, WorkflowIdConflictPolicy, WorkflowIdReusePolicy,
    WorkflowStartOptions,
};
use uuid::Uuid;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RunReceipt {
    pub run_id: Uuid,
    pub workflow_id: String,
}

pub struct ExecutionClient {
    client: Client,
    namespace: String,
    task_queue: String,
}
impl ExecutionClient {
    pub fn schedule_target(
        namespace: &str,
        task_queue: &str,
    ) -> hudson_core::scheduling::ScheduleTarget {
        hudson_core::scheduling::ScheduleTarget {
            scheduler: format!("temporal:{namespace}"),
            task_queue: task_queue.into(),
        }
    }
    pub fn target(&self) -> hudson_core::scheduling::ScheduleTarget {
        Self::schedule_target(&self.namespace, &self.task_queue)
    }

    /// `namespace` is the Hudson PostgreSQL namespace, not the Temporal namespace
    /// (which belongs to the supplied Temporal client). Workers must match it.
    pub fn new(
        client: Client,
        namespace: impl Into<String>,
        task_queue: impl Into<String>,
    ) -> Self {
        Self {
            client,
            namespace: namespace.into(),
            task_queue: task_queue.into(),
        }
    }

    /// Return after Temporal accepts the run. Stable IDs attach duplicate calls
    /// to an existing workflow and never rerun a closed workflow.
    pub async fn start(
        &self,
        run_id: Uuid,
    ) -> Result<RunReceipt, temporalio_client::errors::WorkflowStartError> {
        let workflow_id = format!("hudson:{}:{run_id}", self.namespace);
        let started = self
            .client
            .start_workflow(
                RunWorkflow::run,
                run_id,
                WorkflowStartOptions::new(self.task_queue.clone(), workflow_id.clone())
                    .id_conflict_policy(WorkflowIdConflictPolicy::UseExisting)
                    .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
                    .build(),
            )
            .await;
        match started {
            Ok(_) | Err(temporalio_client::errors::WorkflowStartError::AlreadyStarted { .. }) => {
                Ok(RunReceipt {
                    run_id,
                    workflow_id,
                })
            }
            Err(error) => Err(error),
        }
    }

    /// Foreground wait for a previously submitted run; this performs no model IO.
    pub async fn result(
        &self,
        run_id: Uuid,
    ) -> Result<RunView, Box<dyn std::error::Error + Send + Sync>> {
        let handle = self
            .client
            .get_workflow_handle::<RunWorkflowDefinition>(format!(
                "hudson:{}:{run_id}",
                self.namespace
            ));
        Ok(handle
            .get_result(WorkflowGetResultOptions::default())
            .await?)
    }
}
