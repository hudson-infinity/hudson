//! Retry publication of explicitly scheduled roots without retrying their effects.
use crate::ExecutionClient;
use hudson_core::{
    configured::ConfiguredTree, models::*, scheduling::ScheduleTarget, storage::Store,
};

type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct SchedulingPump {
    store: Option<Store>,
    actor: Actor,
    agent: VersionRef,
    goal: Option<Goal>,
    budget: Option<hudson_core::budgets::ModelBudgetBinding>,
    target: ScheduleTarget,
}
impl SchedulingPump {
    pub fn new(tree: &ConfiguredTree, actor: Actor, target: ScheduleTarget) -> Self {
        Self {
            store: Some(tree.runtime.store.clone()),
            actor,
            agent: tree.reference.clone(),
            goal: tree.goal.clone(),
            budget: tree.runtime.model_budget_binding(),
            target,
        }
    }
    /// Returns the number accepted by Temporal. Failed publication remains pending.
    pub async fn publish_pending(&self, execution: &ExecutionClient) -> Result<usize, Error> {
        if self.target != execution.target() {
            return Err("scheduler target mismatch".into());
        }
        let store = self.store.as_ref().expect("live scheduling pump").clone();
        let actor = self.actor.clone();
        let agent = self.agent.clone();
        let target = self.target.clone();
        let ids = tokio::task::spawn_blocking(move || {
            store.pending_schedules(&actor, &agent, &target, 100)
        })
        .await??;
        let mut published = 0;
        for id in ids {
            let store = self.store.as_ref().expect("live scheduling pump").clone();
            let actor = self.actor.clone();
            let agent = self.agent.clone();
            let goal = self.goal.clone();
            let budget = self.budget.clone();
            tokio::task::spawn_blocking(move || {
                store.validate_resume(&actor, id, &agent, &goal)?;
                store.validate_model_budget(&actor, id, &budget)
            })
            .await??;
            execution.start(id).await?;
            let store = self.store.as_ref().expect("live scheduling pump").clone();
            let actor = self.actor.clone();
            let target = self.target.clone();
            tokio::task::spawn_blocking(move || store.acknowledge_schedule(&actor, id, &target))
                .await??;
            published += 1;
        }
        Ok(published)
    }
}
impl Drop for SchedulingPump {
    fn drop(&mut self) {
        if let Some(store) = self.store.take() {
            if tokio::runtime::Handle::try_current().is_ok() {
                // The final synchronous postgres client owner must drop outside Tokio.
                let _ = std::thread::spawn(move || drop(store)).join();
            } else {
                drop(store);
            }
        }
    }
}
