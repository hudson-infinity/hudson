//! Retry publication of explicitly scheduled roots without retrying their effects.
use crate::ExecutionClient;
use hudson_core::{
    configured::ConfiguredTree, models::*, scheduling::ScheduleTarget, storage::Store,
};

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Default)]
pub struct PublicationReport {
    pub published: usize,
    pub deferred: Vec<(uuid::Uuid, String)>,
}

#[derive(Clone)]
struct ConfiguredBinding {
    agent: VersionRef,
    goal: Option<Goal>,
    budget: Option<hudson_core::budgets::ModelBudgetBinding>,
}

pub struct SchedulingPump {
    store: Option<Store>,
    actor: Actor,
    binding: Option<ConfiguredBinding>,
    target: ScheduleTarget,
}
impl SchedulingPump {
    pub fn new(tree: &ConfiguredTree, actor: Actor, target: ScheduleTarget) -> Self {
        Self {
            store: Some(tree.runtime.store.clone()),
            actor,
            binding: Some(ConfiguredBinding {
                agent: tree.reference.clone(),
                goal: tree.goal.clone(),
                budget: tree.runtime.model_budget_binding(),
            }),
            target,
        }
    }
    pub fn published(store: Store, actor: Actor, target: ScheduleTarget) -> Self {
        Self {
            store: Some(store),
            actor,
            binding: None,
            target,
        }
    }
    /// Failed requests remain pending without preventing other requests from starting.
    pub async fn publish_pending(
        &self,
        execution: &ExecutionClient,
    ) -> Result<PublicationReport, Error> {
        if self.target != execution.target() {
            return Err("scheduler target mismatch".into());
        }
        let store = self.store.as_ref().expect("live scheduling pump").clone();
        let actor = self.actor.clone();
        let binding = self.binding.clone();
        let target = self.target.clone();
        let ids = tokio::task::spawn_blocking(move || match binding {
            Some(binding) => store.pending_schedules(&actor, &binding.agent, &target, 100),
            None => store.pending_published_schedules(&actor, &target, 100),
        })
        .await??;
        let mut report = PublicationReport::default();
        for id in ids {
            match self.publish_one(execution, id).await {
                Ok(()) => report.published += 1,
                Err(error) => report.deferred.push((id, error.to_string())),
            }
        }
        Ok(report)
    }

    async fn publish_one(&self, execution: &ExecutionClient, id: uuid::Uuid) -> Result<(), Error> {
        let store = self.store.as_ref().expect("live scheduling pump").clone();
        let actor = self.actor.clone();
        let binding = self.binding.clone();
        let target = self.target.clone();
        tokio::task::spawn_blocking(move || {
            store.record_schedule_attempt(&actor, id, &target)?;
            let binding = match binding {
                Some(binding) => binding,
                None => {
                    let (_, configuration) = store.published_run_configuration(&actor, id)?;
                    let tree = configuration
                        .build_admission_tree(store.clone(), &actor)
                        .map_err(|error| hudson_core::Error::Invalid(error.to_string()))?;
                    ConfiguredBinding {
                        agent: tree.reference.clone(),
                        goal: tree.goal.clone(),
                        budget: tree.runtime.model_budget_binding(),
                    }
                }
            };
            store.validate_resume(&actor, id, &binding.agent, &binding.goal)?;
            store.validate_model_budget(&actor, id, &binding.budget)
        })
        .await??;
        execution.start(id).await?;
        let store = self.store.as_ref().expect("live scheduling pump").clone();
        let actor = self.actor.clone();
        let target = self.target.clone();
        tokio::task::spawn_blocking(move || store.acknowledge_schedule(&actor, id, &target))
            .await??;
        Ok(())
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
