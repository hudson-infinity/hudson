use hudson_core::{
    configured::Configuration,
    models::{Actor, OperationStatus, RunStatus, WaitReason},
    storage::Store,
};
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use temporalio_sdk::testing::{LocalWorkflowEnvironmentOptions, WorkflowEnvironment};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "requires local PostgreSQL and starts real Temporal and worker processes"]
fn approval_and_killed_write_are_not_replayed_by_temporal() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    tokio::runtime::Runtime::new().unwrap().block_on(async move {
        let env = WorkflowEnvironment::start_local(LocalWorkflowEnvironmentOptions::builder().port(port).build()).await.unwrap();
        tokio::task::spawn_blocking(move || {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let base = format!("http://{}", server.server_addr());
            let file = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(file.path(), serde_json::json!({
                "name":"write-agent","instructions":"write then finish","endpoint":format!("{base}/model"),
                "http_tools":[{"name":"write","description":"test write","endpoint":format!("{base}/write"),"input_schema":{"type":"object"},"effect":"write","require_approval":true}]
            }).to_string()).unwrap();
            let database = std::env::var("HUDSON_TEST_DATABASE").unwrap_or_else(|_| "hudson_harness_test_20260921".into());
            let namespace = format!("temporal-crash-{}", uuid::Uuid::new_v4());
            let actor = Actor { workspace_id:"local".into(), id:"developer".into() };
            let store = Store::postgres_local("/tmp", &database, &namespace).unwrap();
            let tree = Configuration::load(file.path()).unwrap().build_temporal_tree(store.clone(), &actor).unwrap();
            let command = || {
                let mut command = Command::new(env!("CARGO_BIN_EXE_hudson-temporal"));
                command.args(["--config",file.path().to_str().unwrap(),"--database",&database,"--namespace",&namespace,"--task-queue",&namespace])
                    .env("TEMPORAL_ADDRESS",format!("127.0.0.1:{port}")).env("TEMPORAL_NAMESPACE","default")
                    .stdout(Stdio::null()).stderr(Stdio::null());
                command
            };
            let submission = command().args(["run","--task","write","--background"]).stdout(Stdio::piped()).output().unwrap();
            assert!(submission.status.success());
            let receipt: serde_json::Value = serde_json::from_slice(&submission.stdout).unwrap();
            let id = uuid::Uuid::parse_str(receipt["run_id"].as_str().unwrap()).unwrap();
            let mut worker = Process(command().arg("worker").spawn().unwrap());
            let model = server.recv_timeout(Duration::from_secs(20)).unwrap().unwrap();
            assert_eq!(model.url(),"/model");
            model.respond(tiny_http::Response::from_string(r#"{"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"write-1","type":"function","function":{"name":"write","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}"#)).unwrap();
            let deadline = Instant::now()+Duration::from_secs(20);
            let operation_id = loop {
                if let Some(WaitReason::Approval {operation_id}) = store.inspect(&actor,id).unwrap().wait { break operation_id; }
                assert!(Instant::now()<deadline,"approval was not requested");
                std::thread::sleep(Duration::from_millis(50));
            };
            assert!(server.recv_timeout(Duration::from_millis(500)).unwrap().is_none(),"effect executed before approval");
            let now = hudson_core::models::now();
            tree.runtime.approve(&actor,operation_id,true,now,now+120_000).unwrap();
            let write = server.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
            assert_eq!(write.url(),"/write");
            // The destination applied its write, but deliberately holds the
            // receipt. Kill the executor before Hudson can record success.
            worker.0.kill().unwrap();
            worker.0.wait().unwrap();
            drop(write);
            let operation = store.operations(&actor,id).unwrap().into_iter().find(|op| op.meta.id==operation_id).unwrap();
            assert_eq!(operation.status,OperationStatus::Running);
            assert_eq!(operation.attempts.len(),1);
            let replacement = Process(command().arg("worker").spawn().unwrap());
            // Longer than the heartbeat timeout plus initial activity retry.
            assert!(server.recv_timeout(Duration::from_secs(15)).unwrap().is_none(),"Temporal replayed an uncertain effect");
            let operation = store.operations(&actor,id).unwrap().into_iter().find(|op| op.meta.id==operation_id).unwrap();
            assert_eq!(operation.attempts.len(),1);
            tree.runtime.mark_interrupted(&actor,operation_id,operation.attempts[0].id,"test executor SIGKILL and waitpid confirmed").unwrap();
            tree.runtime.record_reconciled_tool_result(&actor,operation_id,serde_json::json!({"written":true}),"test destination observed exactly one write").unwrap();
            let model = server.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
            assert_eq!(model.url(),"/model");
            model.respond(tiny_http::Response::from_string(r#"{"choices":[{"message":{"role":"assistant","content":"done"},"finish_reason":"stop"}]}"#)).unwrap();
            let deadline = Instant::now()+Duration::from_secs(15);
            loop {
                let view = store.inspect(&actor,id).unwrap();
                if view.status.terminal() { assert_eq!(view.status,RunStatus::Completed); break; }
                assert!(Instant::now()<deadline,"run did not complete after reconciliation");
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(server.recv_timeout(Duration::from_millis(200)).unwrap().is_none());
            drop(replacement);
        }).await.unwrap();
        env.shutdown().await.unwrap();
    });
}
