//! Durable sandbox orchestration decisions, independent of transport.
//! Requests must be persisted before admission; unknown outcomes never authorize replay.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Create,
    Execute,
    Destroy,
}

/// Store this alongside the Hudson run before sending any sandbox request.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingOperation {
    pub run_id: String,
    pub workspace_id: String,
    pub step: Step,
    pub idempotency_key: String,
    pub sandbox_id: Option<String>,
    pub operation_id: Option<String>,
    pub admission_attempted: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Receipt {
    pub operation_id: String,
    pub sandbox_id: String,
    pub kind: String,
    pub status: String,
    #[serde(default)]
    pub response_expired: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Exactly one admission attempt is permitted after persisting attempted=true.
    Admit,
    Inspect {
        operation_id: String,
    },
    Wait,
    Succeeded,
    Failed,
    /// Requires explicit reconciliation; never generate a replacement key or execute again.
    Reconcile,
}

impl PendingOperation {
    pub fn decision(&self, receipt: Option<&Receipt>) -> Decision {
        let Some(receipt) = receipt else {
            return match (&self.operation_id, self.admission_attempted) {
                (Some(id), _) => Decision::Inspect {
                    operation_id: id.clone(),
                },
                (None, false) => Decision::Admit,
                (None, true) => Decision::Reconcile,
            };
        };
        let expected_kind = match self.step {
            Step::Create => "create",
            Step::Execute => "execute",
            Step::Destroy => "destroy",
        };
        if self.operation_id.as_deref() != Some(receipt.operation_id.as_str())
            || self.sandbox_id.as_deref() != Some(receipt.sandbox_id.as_str())
            || receipt.kind != expected_kind
            || receipt.response_expired
        {
            return Decision::Reconcile;
        }
        match receipt.status.as_str() {
            "succeeded" => Decision::Succeeded,
            "failed" | "cancelled" => Decision::Failed,
            "queued" | "running" => Decision::Wait,
            _ => Decision::Reconcile,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pending() -> PendingOperation {
        PendingOperation {
            run_id: "run".into(),
            workspace_id: "workspace".into(),
            step: Step::Execute,
            idempotency_key: "stable-key".into(),
            sandbox_id: Some("sandbox".into()),
            operation_id: Some("operation".into()),
            admission_attempted: true,
        }
    }
    fn receipt() -> Receipt {
        Receipt {
            operation_id: "operation".into(),
            sandbox_id: "sandbox".into(),
            kind: "execute".into(),
            status: "succeeded".into(),
            response_expired: false,
        }
    }
    #[test]
    fn current_http_operation_fixture_is_accepted() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/sandbox-operation-schema.json"
        ))
        .unwrap();
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/sandbox-operation.json"))
                .unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&fixture)
            .unwrap();
        let r: Receipt =
            serde_json::from_str(include_str!("../../tests/fixtures/sandbox-operation.json"))
                .unwrap();
        let mut p = pending();
        p.operation_id = Some(r.operation_id.clone());
        p.sandbox_id = Some(r.sandbox_id.clone());
        assert_eq!(p.decision(Some(&r)), Decision::Succeeded);
        for status in ["queued", "running"] {
            let mut r = r.clone();
            r.status = status.into();
            assert_eq!(p.decision(Some(&r)), Decision::Wait);
        }
    }
    #[test]
    fn restart_inspects_original_operation() {
        let original = pending();
        let restored: PendingOperation =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert_eq!(
            restored.decision(None),
            Decision::Inspect {
                operation_id: "operation".into()
            }
        );
        assert_eq!(restored.idempotency_key, original.idempotency_key);
    }
    #[test]
    fn lost_admission_response_cannot_replay() {
        let mut p = pending();
        p.operation_id = None;
        assert_eq!(p.decision(None), Decision::Reconcile);
        p.admission_attempted = false;
        assert_eq!(p.decision(None), Decision::Admit);
    }
    #[test]
    fn receipts_must_match_operation_sandbox_and_kind() {
        let p = pending();
        let good = receipt();
        assert_eq!(p.decision(Some(&good)), Decision::Succeeded);
        for field in 0..3 {
            let mut bad = receipt();
            match field {
                0 => bad.operation_id = "other".into(),
                1 => bad.sandbox_id = "other".into(),
                _ => bad.kind = "destroy".into(),
            }
            assert_eq!(p.decision(Some(&bad)), Decision::Reconcile);
        }
    }
    #[test]
    fn expired_unknown_or_new_status_never_replays() {
        let p = pending();
        for status in ["unknown", "future_status"] {
            let mut r = receipt();
            r.status = status.into();
            assert_eq!(p.decision(Some(&r)), Decision::Reconcile);
        }
        let mut r = receipt();
        r.response_expired = true;
        assert_eq!(p.decision(Some(&r)), Decision::Reconcile);
    }
}
