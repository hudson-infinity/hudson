//! Temporal schedules the existing Hudson runtime; workflows never perform effects.
pub mod client;
pub mod scheduling;
pub use client::{ExecutionClient, RunReceipt};
pub use scheduling::SchedulingPump;

use hudson_core::{
    configured::ScheduledTree,
    models::{Actor, OperationStatus, RunStatus, RunView},
};
use std::{sync::Arc, time::Duration};
use temporalio_macros::{activities, workflow, workflow_methods};
use temporalio_sdk::{
    activities::{ActivityContext, ActivityError},
    ActivityOptions, ChildWorkflowOptions, ContinueAsNewOptions, WorkflowContext, WorkflowResult,
};
use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TickResult {
    pub view: RunView,
    pub children: Vec<Uuid>,
}

pub struct RunActivities {
    tree: Option<ScheduledTree>,
    actor: Actor,
}

impl RunActivities {
    pub fn new(tree: ScheduledTree, actor: Actor) -> Self {
        Self {
            tree: Some(tree),
            actor,
        }
    }
}
impl Drop for RunActivities {
    fn drop(&mut self) {
        if let Some(tree) = self.tree.take() {
            // postgres::Client performs blocking IO even during Drop. Worker
            // teardown happens inside Tokio, so close it on a plain thread.
            if tokio::runtime::Handle::try_current().is_ok() {
                let _ = std::thread::spawn(move || drop(tree)).join();
            } else {
                drop(tree);
            }
        }
    }
}

#[activities]
impl RunActivities {
    /// Runtime persists operation intent before IO and refuses to replay an
    /// already-running or unknown operation, including after activity retries.
    #[activity]
    pub async fn tick(
        self: Arc<Self>,
        ctx: ActivityContext,
        id: Uuid,
    ) -> Result<TickResult, ActivityError> {
        let mut task = tokio::task::spawn_blocking(move || {
            let tree = self.tree.as_ref().expect("live activity tree");
            let pending = tree
                .store
                .operations(&self.actor, id)
                .map_err(|e| e.to_string())?
                .iter()
                .any(|op| {
                    matches!(
                        op.status,
                        OperationStatus::Pending
                            | OperationStatus::WaitingApproval
                            | OperationStatus::Running
                            | OperationStatus::Unknown
                    )
                });
            let children = tree
                .store
                .children(&self.actor, id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|child| !child.status.terminal())
                .map(|child| child.id)
                .collect::<Vec<_>>();
            // Finish dispatching a batch before joining its children. Never let
            // the lead ask its model to continue while its team is still working.
            if !pending && !children.is_empty() {
                let view = tree
                    .store
                    .inspect(&self.actor, id)
                    .map_err(|e| e.to_string())?;
                return Ok(TickResult { view, children });
            }
            let view = tree.tick(&self.actor, id).map_err(|e| e.to_string())?;
            Ok(TickResult {
                view,
                children: Vec::new(),
            })
        });
        let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
        loop {
            tokio::select! {
                result = &mut task => return result
                    .map_err(|e| ActivityError::from(anyhow_error(e.to_string())))?
                    .map_err(|e| ActivityError::from(anyhow_error(e))),
                _ = heartbeat.tick() => { ctx.record_heartbeat(id).await?; }
            }
        }
    }
}

fn anyhow_error(message: String) -> std::io::Error {
    std::io::Error::other(message)
}

#[workflow]
#[derive(Default)]
pub struct RunWorkflow;

#[workflow_methods]
impl RunWorkflow {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, id: Uuid) -> WorkflowResult<RunView> {
        for _ in 0..500 {
            let tick = ctx
                .execute_activity(
                    RunActivities::tick,
                    id,
                    ActivityOptions::with_start_to_close_timeout(Duration::from_secs(180))
                        .heartbeat_timeout(Duration::from_secs(10))
                        .build(),
                )
                .await?;
            let view = tick.view;
            let mut children = Vec::new();
            for child in tick.children {
                children.push(
                    ctx.start_child_workflow(
                        RunWorkflow::run,
                        child,
                        ChildWorkflowOptions::workflow_id(format!("hudson-child-{child}")),
                    )
                    .await?,
                );
            }
            // Start the entire team before waiting, so siblings can execute on
            // independent worker activities rather than nested tool callbacks.
            for child in children {
                child.result().await?;
            }
            if view.status.terminal() {
                return Ok(view);
            }
            // A durable timer also discovers approvals and input recorded by
            // the existing control API without depending on a live HTTP host.
            if matches!(view.status, RunStatus::Waiting | RunStatus::Cancelling) {
                ctx.timer(Duration::from_secs(5)).await;
            } else {
                ctx.timer(Duration::from_millis(100)).await;
            }
        }
        ctx.continue_as_new(id, ContinueAsNewOptions::default())?;
        unreachable!("continue as new exits the workflow")
    }
}

pub use run_workflow::Run as RunWorkflowDefinition;
