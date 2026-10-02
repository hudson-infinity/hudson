//! Pinned public SDK qualification. No mutation dispatch or runtime integration.
//! Observations lack verified operator-profile identity and are not completion evidence.
use hudson_core::adapters::sandbox_recovery::{Decision, PendingOperation, Receipt, Step};
pub use sandbox_client::models::{CommandInput, CreateRequest, DestroyRequest};
use sandbox_client::{requests::GetOperation, Client};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid or noncanonical prepared request")]
    Request,
    #[error("original operation requires reconciliation")]
    Reconcile,
    #[error("SDK cannot verify the durable operator profile identity")]
    ProfileIdentityUnavailable,
    #[error("observation exceeds caller payload bound")]
    Oversized,
    #[error(transparent)]
    Sdk(#[from] sandbox_client::Error),
}

/// Canonical SDK wire body; no credentials or inferred package/command mapping.
pub enum PreparedBody {
    Create(CreateRequest),
    Execute(CommandInput),
    Destroy(DestroyRequest),
}
impl PreparedBody {
    pub fn step(&self) -> Step {
        match self {
            Self::Create(_) => Step::Create,
            Self::Execute(_) => Step::Execute,
            Self::Destroy(_) => Step::Destroy,
        }
    }
    /// Persist these exact bytes before a future authorized SDK admission call.
    pub fn canonical_json(&self) -> Result<String, Error> {
        let body = match self {
            Self::Create(value) => serde_json::to_string(value),
            Self::Execute(value) => serde_json::to_string(value),
            Self::Destroy(value) => serde_json::to_string(value),
        }
        .map_err(|_| Error::Request)?;
        if body.len() > 65536 {
            return Err(Error::Request);
        }
        Ok(body)
    }
    /// Reject whitespace/default/unknown-field normalization of existing durable bytes.
    pub fn from_canonical_json(step: Step, body: &str) -> Result<Self, Error> {
        if body.len() > 65536 {
            return Err(Error::Request);
        }
        let parsed = match step {
            Step::Create => Self::Create(serde_json::from_str(body).map_err(|_| Error::Request)?),
            Step::Execute => Self::Execute(serde_json::from_str(body).map_err(|_| Error::Request)?),
            Step::Destroy => Self::Destroy(serde_json::from_str(body).map_err(|_| Error::Request)?),
        };
        if parsed.canonical_json()? != body {
            return Err(Error::Request);
        }
        Ok(parsed)
    }
}

/// Non-authoritative SDK protocol probe. This does not establish endpoint/project binding.
/// Do not attach its results as durable completion or cleanup evidence.
pub struct QualificationObserver {
    client: Client,
}
/// Deliberately no Debug: backend result/error values may contain workload secrets.
pub struct ProbeObservation {
    pub decision: Decision,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub output_status: Option<String>,
}
impl QualificationObserver {
    /// Reads trusted private SDK configuration; no agent arguments or insecure TLS switch.
    pub fn from_operator_profile(path: &Path) -> Result<Self, Error> {
        Ok(Self {
            client: Client::from_config(path)?,
        })
    }
    /// Fail closed before polling: the pinned SDK does not expose actual profile identity.
    pub async fn observe_binding(
        &self,
        _binding: &hudson_core::sandbox_binding::Binding,
    ) -> Result<ProbeObservation, Error> {
        Err(Error::ProfileIdentityUnavailable)
    }
    /// Inspects only the original bound ID. Never waits, cancels, retries, or mutates.
    /// Profile identity is unverified until the SDK exposes its canonical origin/project.
    pub async fn inspect_unverified(
        &self,
        pending: &PendingOperation,
        max_payload_bytes: usize,
    ) -> Result<ProbeObservation, Error> {
        if !(1..=65536).contains(&max_payload_bytes) {
            return Err(Error::Request);
        }
        let Decision::Inspect { operation_id } = pending.decision(None) else {
            return Err(Error::Reconcile);
        };
        let operation = self
            .client
            .get_operation(GetOperation {
                operation_id: &operation_id,
            })
            .await?;
        if serde_json::to_vec(&operation)
            .map_err(|_| Error::Request)?
            .len()
            > max_payload_bytes
        {
            return Err(Error::Oversized);
        }
        let receipt = Receipt {
            operation_id: operation.operation_id,
            sandbox_id: operation.sandbox_id,
            kind: operation.kind,
            status: operation.status,
            response_expired: operation.response_expired,
        };
        let decision = pending.decision(Some(&receipt));
        // Unknown/expired/mismatched receipts cannot smuggle an apparent success result.
        let release = matches!(decision, Decision::Succeeded | Decision::Failed);
        Ok(ProbeObservation {
            decision,
            result: if release { operation.result } else { None },
            error: if release { operation.error } else { None },
            output_status: operation.output_status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_bytes_preserve_absolute_deadline_and_reject_normalization() {
        let request = PreparedBody::Execute(CommandInput {
            argv: vec!["/approved/agent".into()],
            env: Default::default(),
            cwd: "/workspace".into(),
            deadline_unix_ms: 123456789,
            output_limit: 4096,
        });
        let body = request.canonical_json().unwrap();
        assert!(body.contains("123456789"));
        assert_eq!(
            PreparedBody::from_canonical_json(Step::Execute, &body)
                .unwrap()
                .canonical_json()
                .unwrap(),
            body
        );
        assert!(PreparedBody::from_canonical_json(Step::Execute, &format!(" {body}")).is_err());
        assert!(PreparedBody::from_canonical_json(Step::Destroy, "{}").is_err());
        assert!(PreparedBody::from_canonical_json(Step::Create, "{\"unknown\":true}").is_err());
    }
}
