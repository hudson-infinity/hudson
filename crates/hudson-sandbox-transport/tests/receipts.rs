//! Local trusted-CA SDK protocol fixture; no sandbox runtime.
use hudson_core::adapters::sandbox_recovery::{Decision, PendingOperation, Step};
use hudson_sandbox_transport::{Error, QualificationObserver};
use std::{os::unix::fs::PermissionsExt, sync::Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{rustls, TlsAcceptor};
fn private(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
async fn probe(
    status: u16,
    body: serde_json::Value,
    bound: usize,
) -> Result<hudson_sandbox_transport::ProbeObservation, Error> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.cert.der().clone()],
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                cert.signing_key.serialize_der(),
            )),
        )
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut tls = TlsAcceptor::from(Arc::new(tls))
            .accept(socket)
            .await
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 1024];
        while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = tls.read(&mut buffer).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buffer[..n]);
            assert!(bytes.len() < 65536);
        }
        let headers = std::str::from_utf8(&bytes).unwrap().to_ascii_lowercase();
        assert!(headers.starts_with("get /v1/operations/op_original http/1.1"));
        assert!(headers.contains("authorization: bearer fixture-only"));
        assert!(!headers.contains("idempotency-key:"));
        let body = body.to_string();
        tls.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        tls.shutdown().await.unwrap();
    });
    let directory = tempfile::tempdir().unwrap();
    private(&directory.path().join("ca.pem"), cert.cert.pem().as_bytes());
    private(&directory.path().join("credential.json"),br#"{"version":1,"project_id":"prj_fixture","name":"test","token":"fixture-only","created_at":1,"expires_at":4102444800}"#);
    let profile = directory.path().join("client.json");
    private(&profile,&serde_json::to_vec(&serde_json::json!({"version":1,"endpoint":format!("https://localhost:{port}"),"credential_file":"credential.json","ca_file":"ca.pem"})).unwrap());
    let observer = QualificationObserver::from_operator_profile(&profile).unwrap();
    let pending = PendingOperation {
        run_id: "hudson-run".into(),
        workspace_id: "hudson-workspace".into(),
        step: Step::Execute,
        idempotency_key: "stable-key-000001".into(),
        sandbox_id: Some("sb_original".into()),
        operation_id: Some("op_original".into()),
        admission_attempted: true,
    };
    let result = observer.inspect_unverified(&pending, bound).await;
    task.await.unwrap();
    result
}
fn receipt(status: &str) -> serde_json::Value {
    serde_json::json!({"operation_id":"op_original","sandbox_id":"sb_original","kind":"execute","status":status,"created_at":"2026-10-02T00:00:00Z","result":{"exit_code":0},"output_status":"complete"})
}
#[tokio::test]
async fn original_receipt_and_unknown_expired_mismatched_results() {
    for mode in [
        "succeeded",
        "queued",
        "unknown",
        "expired",
        "mismatch",
        "wrong_operation",
        "wrong_kind",
    ] {
        let mut body = receipt(if mode == "queued" {
            "queued"
        } else if mode == "unknown" {
            "unknown"
        } else {
            "succeeded"
        });
        if mode == "expired" {
            body["response_expired"] = true.into();
        }
        if mode == "mismatch" {
            body["sandbox_id"] = "sb_other".into();
        }
        if mode == "wrong_operation" {
            body["operation_id"] = "op_other".into();
        }
        if mode == "wrong_kind" {
            body["kind"] = "destroy".into();
        }
        let observed = probe(200, body, 4096).await.unwrap();
        if mode == "succeeded" {
            assert_eq!(observed.decision, Decision::Succeeded);
            assert!(observed.result.is_some());
        } else {
            assert_eq!(
                observed.decision,
                if mode == "queued" {
                    Decision::Wait
                } else {
                    Decision::Reconcile
                }
            );
            assert!(observed.result.is_none());
        }
    }
}
#[tokio::test]
async fn expired_http_and_oversized_results_are_not_completion() {
    assert!(matches!(probe(410,serde_json::json!({"status":410,"title":"expired","code":"response_expired","operation_id":"op_original"}),4096).await,Err(Error::Sdk(sandbox_client::Error::Http {status:410,..}))));
    assert!(matches!(
        probe(200, receipt("succeeded"), 1).await,
        Err(Error::Oversized)
    ));
}

#[tokio::test]
async fn authoritative_observation_and_unbound_plans_fail_before_network() {
    // Port 1 has no fixture: these branches must return locally without a GET.
    let directory = tempfile::tempdir().unwrap();
    private(&directory.path().join("credential.json"), br#"{"version":1,"project_id":"prj_fixture","name":"test","token":"fixture-only","created_at":1,"expires_at":4102444800}"#);
    let profile = directory.path().join("client.json");
    private(
        &profile,
        br#"{"version":1,"endpoint":"https://localhost:1","credential_file":"credential.json"}"#,
    );
    let observer = QualificationObserver::from_operator_profile(&profile).unwrap();
    let binding: hudson_core::sandbox_binding::Binding = serde_json::from_value(serde_json::json!({
        "fence":{"run_id":"00000000-0000-0000-0000-000000000001","operation_id":"00000000-0000-0000-0000-000000000002","attempt_id":"00000000-0000-0000-0000-000000000003"},
        "operation_request_digest":"fixture", "body":"{}", "profile_fingerprint":"a".repeat(64),
        "pending":{"run_id":"run","workspace_id":"workspace","step":"execute","idempotency_key":"stable-key-000001","sandbox_id":"sb_original","operation_id":null,"admission_attempted":true},
        "initial_pending":{"run_id":"run","workspace_id":"workspace","step":"execute","idempotency_key":"stable-key-000001","sandbox_id":"sb_original","operation_id":null,"admission_attempted":false},
        "terminal":null
    })).unwrap();
    assert!(matches!(
        observer.observe_binding(&binding).await,
        Err(Error::ProfileIdentityUnavailable)
    ));
    assert!(matches!(
        observer.inspect_unverified(&binding.pending, 4096).await,
        Err(Error::Reconcile)
    ));
    assert!(matches!(
        observer
            .inspect_unverified(&binding.initial_pending, 4096)
            .await,
        Err(Error::Reconcile)
    ));
}
