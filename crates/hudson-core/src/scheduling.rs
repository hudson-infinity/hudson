//! Durable scheduling intent. Remote schedulers acknowledge only after accepting
//! the stable run identity; retrying publication must not replay execution.
use crate::{models::*, storage::Store, Error, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScheduleTarget {
    pub scheduler: String,
    pub task_queue: String,
}
impl ScheduleTarget {
    pub fn validate(&self) -> Result<()> {
        if [&self.scheduler, &self.task_queue]
            .iter()
            .any(|v| v.trim().is_empty() || v.len() > 256)
        {
            return Err(Error::Invalid("invalid scheduling target".into()));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ScheduleRequest {
    pub target: ScheduleTarget,
    pub acknowledged: bool,
    #[serde(default)]
    pub last_attempt: u64,
}
impl Store {
    /// Check that a run (or its root for delegated work) belongs to this scheduler.
    /// This is also required before accepting controls from a scheduling-only host.
    pub fn validate_schedule(
        &self,
        actor: &Actor,
        id: Uuid,
        target: Option<&ScheduleTarget>,
    ) -> Result<()> {
        self.read(|data| {
            let mut current = id;
            let mut visited = std::collections::BTreeSet::new();
            loop {
                if !visited.insert(current) {
                    return Err(Error::Conflict("cyclic run ancestry".into()));
                }
                let run = data.run(actor, current)?;
                match run.parent_operation {
                    Some(operation) => {
                        current = data
                            .operations
                            .get(&operation)
                            .ok_or(Error::NotFound)?
                            .run_id;
                    }
                    None => break,
                }
            }
            if data
                .schedule_requests
                .get(&current)
                .map(|request| &request.target)
                == target
            {
                Ok(())
            } else {
                Err(Error::Conflict(
                    "run belongs to a different execution host".into(),
                ))
            }
        })
    }

    /// Return only explicitly scheduled roots owned by this actor and agent.
    pub fn pending_schedules(
        &self,
        actor: &Actor,
        agent: &VersionRef,
        target: &ScheduleTarget,
        limit: usize,
    ) -> Result<Vec<Uuid>> {
        target.validate()?;
        if !(1..=100).contains(&limit) {
            return Err(Error::Invalid(
                "scheduling batch must contain 1 to 100 runs".into(),
            ));
        }
        self.read(|data| {
            let mut pending = Vec::new();
            for (id, request) in &data.schedule_requests {
                if request.acknowledged || &request.target != target {
                    continue;
                }
                let Ok(run) = data.run(actor, *id) else {
                    continue;
                };
                if run.agent_ref == *agent
                    && run.parent_operation.is_none()
                    && !run.status.terminal()
                {
                    pending.push((request.last_attempt, run.meta.created_at, *id));
                }
            }
            pending.sort_unstable();
            Ok(pending
                .into_iter()
                .take(limit)
                .map(|(_, _, id)| id)
                .collect())
        })
    }
    /// Rotate attempted requests behind untouched work, including invalid requests.
    pub fn record_schedule_attempt(
        &self,
        actor: &Actor,
        id: Uuid,
        target: &ScheduleTarget,
    ) -> Result<()> {
        self.transact(|data| {
            data.run(actor, id)?;
            let request = data.schedule_requests.get_mut(&id).ok_or(Error::NotFound)?;
            if &request.target != target {
                return Err(Error::Conflict("scheduling target changed".into()));
            }
            request.last_attempt = now();
            Ok(())
        })
    }

    /// Trusted scheduler receipt, never exposed as a model tool.
    pub fn acknowledge_schedule(
        &self,
        actor: &Actor,
        id: Uuid,
        target: &ScheduleTarget,
    ) -> Result<()> {
        self.transact(|data| {
            data.run(actor, id)?;
            let request = data.schedule_requests.get_mut(&id).ok_or(Error::NotFound)?;
            if &request.target != target {
                return Err(Error::Conflict("scheduling target changed".into()));
            }
            request.acknowledged = true;
            Ok(())
        })
    }
}
