use crate::{models::*, storage::MemoryStore, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    pub actors: BTreeSet<String>,
    pub approvers: BTreeSet<String>,
    pub require_approval: bool,
}

impl MemoryStore {
    /// Startup publication that cannot overwrite an existing policy. Explicit
    /// administrators may still use set_policy for intentional revocation.
    pub fn ensure_policy(&self, workspace: &str, id: &str, policy: Policy) -> Result<()> {
        self.transact(|d| {
            let key = (workspace.to_owned(), id.to_owned());
            match d.policies.get(&key) {
                Some(existing) if existing != &policy => Err(Error::Conflict(
                    "policy configuration changed; use a new version or explicit policy update"
                        .into(),
                )),
                Some(_) => Ok(()),
                None => {
                    d.policies.insert(key, policy);
                    Ok(())
                }
            }
        })
    }

    /// Trusted configuration API, deliberately absent from the preview HTTP surface.
    pub fn set_policy(&self, workspace: &str, id: &str, policy: Policy) -> Result<()> {
        self.transact(|d| {
            d.policies.insert((workspace.into(), id.into()), policy);
            Ok(())
        })
    }
}

pub(crate) enum Admission {
    Allow,
    Wait,
    Deny,
}

pub(crate) fn admit(
    policy: Option<&Policy>,
    actor: &str,
    operation: &Operation,
    at: u64,
) -> Admission {
    let Some(policy) = policy else {
        return Admission::Deny;
    };
    if !policy.actors.contains(actor) {
        return Admission::Deny;
    }
    if !policy.require_approval {
        return Admission::Allow;
    }
    let valid = operation
        .approval
        .as_ref()
        .filter(|a| a.request_digest == operation.request_digest)
        .and_then(|a| a.decisions.last())
        .is_some_and(|d| {
            d.approved
                && d.request_digest == operation.request_digest
                && d.decided_at <= at
                && at < d.expires_at
                && policy.approvers.contains(&d.actor_id)
        });
    if valid {
        Admission::Allow
    } else {
        Admission::Wait
    }
}

pub(crate) fn require_workspace(actor: &Actor, workspace: &str) -> Result<()> {
    if actor.workspace_id != workspace {
        return Err(Error::NotFound);
    }
    Ok(())
}
