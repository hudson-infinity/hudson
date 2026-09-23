//! Reconstruct immutable publications on demand using worker-side credentials.
use crate::{
    configured::ScheduledTree, models::*, scheduling::ScheduleTarget, storage::Store, Error, Result,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// One owner's published revisions on one scheduler target. Construct and drop on
/// blocking threads, like the configured runtime and synchronous PostgreSQL store.
pub struct PublishedWorker {
    pub store: Store,
    actor: Actor,
    target: ScheduleTarget,
    trees: Mutex<BTreeMap<VersionRef, Arc<ScheduledTree>>>,
}
impl PublishedWorker {
    pub fn new(store: Store, actor: Actor, target: ScheduleTarget) -> Result<Self> {
        target.validate()?;
        if actor.workspace_id.trim().is_empty() || actor.id.trim().is_empty() {
            return Err(Error::Denied);
        }
        Ok(Self {
            store,
            actor,
            target,
            trees: Mutex::new(BTreeMap::new()),
        })
    }
    pub fn validate_run(&self, id: uuid::Uuid) -> Result<VersionRef> {
        self.store
            .validate_schedule(&self.actor, id, Some(&self.target))?;
        self.store.published_root(&self.actor, id)
    }
    pub fn tick(&self, id: uuid::Uuid) -> Result<RunView> {
        let root = self.validate_run(id)?;
        let tree = {
            let mut trees = self
                .trees
                .lock()
                .map_err(|_| Error::Conflict("published runtime unavailable".into()))?;
            if let Some(tree) = trees.get(&root) {
                tree.clone()
            } else {
                let tree = Arc::new(
                    self.store
                        .published_configuration(&self.actor, &root)?
                        .build_temporal_tree(self.store.clone(), &self.actor)
                        .map_err(|error| Error::Invalid(error.to_string()))?
                        .into_scheduled(),
                );
                // Bound cached revisions. In-flight trees retain their own Arc while
                // evicted; the runtime's durable operation claim still owns effects.
                if trees.len() >= 64 {
                    if let Some(evicted) = trees.keys().next().cloned() {
                        trees.remove(&evicted);
                    }
                }
                trees.insert(root, tree.clone());
                tree
            }
        };
        tree.tick(&self.actor, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configured::Configuration;
    use serde_json::json;

    #[test]
    fn published_queue_and_worker_reject_unowned_unscheduled_and_wrong_queue_runs() {
        let actor = Actor {
            workspace_id: "company".into(),
            id: "owner".into(),
        };
        let target = ScheduleTarget {
            scheduler: "temporal:test".into(),
            task_queue: "queue".into(),
        };
        let store = Store::default();
        let worker = PublishedWorker::new(store.clone(), actor.clone(), target.clone()).unwrap();
        let mut valid = vec![];
        for version in [1, 2] {
            let path = std::env::temp_dir()
                .join(format!("published-worker-{}.json", uuid::Uuid::new_v4()));
            std::fs::write(&path, json!({"name":"agent","version":version,"instructions":"help","provider":"ollama","model":"fixture"}).to_string()).unwrap();
            let config = Configuration::load(&path).unwrap();
            std::fs::remove_file(path).unwrap();
            store
                .publish_configuration(&actor, &config, &format!("publish-{version}"))
                .unwrap();
            let tree = config.build_admission_tree(store.clone(), &actor).unwrap();
            let submit = |actor: &Actor, target| {
                tree.runtime
                    .submit_scheduled(
                        actor,
                        tree.reference.clone(),
                        json!("task"),
                        None,
                        None,
                        target,
                    )
                    .unwrap()
            };
            let id = submit(&actor, Some(target.clone()));
            valid.push(id);
            assert_eq!(worker.validate_run(id).unwrap(), tree.reference);
            let other_queue = submit(
                &actor,
                Some(ScheduleTarget {
                    task_queue: "other".into(),
                    ..target.clone()
                }),
            );
            let local = submit(&actor, None);
            assert!(worker.tick(other_queue).is_err());
            assert!(worker.tick(local).is_err());
            let other = Actor {
                id: "other".into(),
                ..actor.clone()
            };
            // A trusted runtime can admit another actor to an existing definition,
            // but the publication owner is still required for dynamic execution.
            let foreign = submit(&other, Some(target.clone()));
            assert!(worker.tick(foreign).is_err());
            assert!(store
                .pending_published_schedules(&other, &target, 100)
                .unwrap()
                .is_empty());
            let foreign_worker =
                PublishedWorker::new(store.clone(), other, target.clone()).unwrap();
            assert!(foreign_worker.tick(foreign).is_err());
            assert!(store.operations(&actor, local).unwrap().is_empty());
            tree.runtime.cancel(&actor, id).unwrap();
            assert_eq!(worker.tick(id).unwrap().status, RunStatus::Cancelled);
        }
        assert_eq!(valid.len(), 2);
        assert!(store
            .pending_published_schedules(&actor, &target, 100)
            .unwrap()
            .is_empty());
    }
}
