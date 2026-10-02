//! Serialized sandbox recovery decisions, independent of transport and storage.
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

/// Public v1 202 body. Admission is acceptance, never completion evidence.
#[derive(Clone, Debug, Deserialize)]
pub struct Admission {
    pub sandbox_id: String,
    pub operation_id: String,
    pub status: String,
    pub status_url: String,
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
    /// Bind a successful admission response after the original send intent was persisted.
    /// Error bodies (including 409/410) never establish admission or completion.
    pub fn admitted(&self, http_status: u16, body: &Admission) -> Option<Self> {
        if (self.step != Step::Create && self.sandbox_id.is_none())
            || http_status != 202
            || !self.admission_attempted
            || body.sandbox_id.is_empty()
            || body.operation_id.is_empty()
            || body.status_url != format!("/v1/operations/{}", body.operation_id)
            || self
                .sandbox_id
                .as_ref()
                .is_some_and(|id| id != &body.sandbox_id)
            || self
                .operation_id
                .as_ref()
                .is_some_and(|id| id != &body.operation_id)
        {
            return None;
        }
        let mut bound = self.clone();
        bound.sandbox_id = Some(body.sandbox_id.clone());
        bound.operation_id = Some(body.operation_id.clone());
        // Reuse structural validation. The response's status is not a completion receipt.
        if bound.decision(None) == Decision::Reconcile {
            return None;
        }
        Some(bound)
    }

    /// This checks structural consistency only. Ownership and immutable request binding
    /// must be checked by the durable store before calling this method.
    pub fn decision(&self, receipt: Option<&Receipt>) -> Decision {
        if self.run_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || !(16..=128).contains(&self.idempotency_key.len())
            || !self
                .idempotency_key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || self
                .operation_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
            || self
                .sandbox_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
            || (self.step != Step::Create && self.sandbox_id.is_none())
            || (self.operation_id.is_some()
                && (!self.admission_attempted || self.sandbox_id.is_none()))
        {
            return Decision::Reconcile;
        }
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
            idempotency_key: "stable-key-000001".into(),
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
    fn admission_is_only_acceptance_and_errors_never_bind() {
        let body = Admission {
            sandbox_id: "sandbox".into(),
            operation_id: "operation".into(),
            status: "succeeded".into(),
            status_url: "/v1/operations/operation".into(),
        };
        let mut p = pending();
        p.operation_id = None;
        let bound = p.admitted(202, &body).unwrap();
        assert_eq!(
            bound.decision(None),
            Decision::Inspect {
                operation_id: "operation".into()
            }
        );
        for status in [200, 409, 410, 500] {
            assert!(p.admitted(status, &body).is_none());
        }
        let mut wrong = body.clone();
        wrong.sandbox_id = "other".into();
        assert!(p.admitted(202, &wrong).is_none());
    }
    #[test]
    fn admission_cannot_invent_original_execute_or_destroy_target() {
        let body = Admission {
            sandbox_id: "sandbox".into(),
            operation_id: "operation".into(),
            status: "queued".into(),
            status_url: "/v1/operations/operation".into(),
        };
        for step in [Step::Execute, Step::Destroy] {
            let mut p = pending();
            p.step = step;
            p.sandbox_id = None;
            p.operation_id = None;
            assert_eq!(p.decision(None), Decision::Reconcile);
            assert!(p.admitted(202, &body).is_none());
        }
        let mut create = pending();
        create.step = Step::Create;
        create.sandbox_id = None;
        create.operation_id = None;
        assert!(create.admitted(202, &body).is_some());
    }
    #[test]
    fn malformed_plans_never_admit() {
        for field in 0..5 {
            let mut p = pending();
            p.operation_id = None;
            p.admission_attempted = false;
            match field {
                0 => p.run_id.clear(),
                1 => p.workspace_id.clear(),
                2 => p.idempotency_key.clear(),
                3 => p.sandbox_id = None,
                _ => p.sandbox_id = Some(String::new()),
            }
            assert_eq!(p.decision(None), Decision::Reconcile);
        }
        for key in ["too-short", "invalid key-000000", "nonascii-é-00000"] {
            let mut p = pending();
            p.operation_id = None;
            p.admission_attempted = false;
            p.idempotency_key = key.into();
            assert_eq!(p.decision(None), Decision::Reconcile);
        }
        let mut p = pending();
        p.idempotency_key = "x".repeat(129);
        assert_eq!(p.decision(None), Decision::Reconcile);
        let mut p = pending();
        p.admission_attempted = false;
        assert_eq!(p.decision(Some(&receipt())), Decision::Reconcile);
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
