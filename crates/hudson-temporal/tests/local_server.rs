use hudson_core::{
    configured::Configuration,
    models::{Actor, RunStatus},
    storage::Store,
};
use hudson_temporal::{RunActivities, RunWorkflow};

use temporalio_client::{WorkflowGetResultOptions, WorkflowStartOptions};
use temporalio_sdk::{
    testing::{LocalWorkflowEnvironmentOptions, WorkflowEnvironment},
    Runtime, Worker, WorkerOptions,
};

#[test]
#[ignore = "starts a local Temporal server (downloads the CLI on first use)"]
fn temporal_runs_existing_runtime_activity() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"{"name":"test","instructions":"help","endpoint":"http://127.0.0.1:1/chat"}"#,
    )
    .unwrap();
    let actor = Actor {
        workspace_id: "local".into(),
        id: "developer".into(),
    };
    let tree = Configuration::load(file.path())
        .unwrap()
        .build_tree(Store::default(), &actor)
        .unwrap();
    let id = tree
        .runtime
        .submit(
            &actor,
            tree.reference.clone(),
            serde_json::json!("test"),
            None,
        )
        .unwrap();
    tree.runtime.cancel(&actor, id).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let env = WorkflowEnvironment::start_local(LocalWorkflowEnvironmentOptions::default())
                .await
                .unwrap();
            let runtime = Runtime::from_current_tokio(Default::default()).unwrap();
            let options = WorkerOptions::new("hudson-test")
                .register_workflow::<RunWorkflow>()
                .unwrap()
                .register_activities(RunActivities::new(tree.into_scheduled(), actor))
                .build();
            let mut worker = Worker::new(&runtime, env.client().clone(), options).unwrap();
            let stop = worker.shutdown_handle();
            let handle = env
                .client()
                .start_workflow(
                    RunWorkflow::run,
                    id,
                    WorkflowStartOptions::new("hudson-test", format!("test-{id}")).build(),
                )
                .await
                .unwrap();
            let (worker_result, result) = tokio::join!(worker.run(), async {
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    handle.get_result(WorkflowGetResultOptions::default()),
                )
                .await;
                stop();
                result
            });
            worker_result.unwrap();
            env.shutdown().await.unwrap();
            assert_eq!(result.unwrap().unwrap().status, RunStatus::Cancelled);
        });
}

#[test]
#[ignore = "starts a local Temporal server (downloads the CLI on first use)"]
fn team_children_run_concurrently_before_lead_continues() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/chat", server.server_addr());
    let barrier = Arc::new(AtomicUsize::new(0));
    let child_calls = Arc::new(AtomicUsize::new(0));
    let children_done = child_calls.clone();
    let server_thread = std::thread::spawn(move || {
        let mut root_calls = 0;
        let mut handlers = Vec::new();
        for _ in 0..5 {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(40))
                .unwrap()
                .expect("model call timed out");
            let body: serde_json::Value = serde_json::from_reader(request.as_reader()).unwrap();
            let is_root = body["model"] == "root-model";
            let first = is_root && root_calls == 0;
            let join = is_root && root_calls == 1;
            if is_root {
                root_calls += 1;
            }
            let barrier = barrier.clone();
            let child_calls = child_calls.clone();
            handlers.push(std::thread::spawn(move || {
                let message = if first {
                    serde_json::json!({"role":"assistant","content":null,"tool_calls":[
                        {"id":"a","type":"function","function":{"name":"delegate_a","arguments":"{\"task\":\"do a\"}"}},
                        {"id":"b","type":"function","function":{"name":"delegate_b","arguments":"{\"task\":\"do b\"}"}}
                    ]})
                } else if join {
                    assert_eq!(child_calls.load(Ordering::SeqCst), 2);
                    let calls = ["a", "b"].into_iter().map(|name| {
                        let receipt = body["messages"].as_array().unwrap().iter()
                            .find(|message| message["role"] == "tool" && message["tool_call_id"] == name).unwrap();
                        let receipt: serde_json::Value = serde_json::from_str(receipt["content"].as_str().unwrap()).unwrap();
                        serde_json::json!({"id":format!("join-{name}"),"type":"function","function":{
                            "name":format!("join_delegate_{name}"),"arguments":serde_json::json!({"run_id":receipt["value"]["child_run"]}).to_string()
                        }})
                    }).collect::<Vec<_>>();
                    serde_json::json!({"role":"assistant","content":null,"tool_calls":calls})
                } else {
                    if !is_root {
                        barrier.fetch_add(1, Ordering::SeqCst);
                        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                        while barrier.load(Ordering::SeqCst) < 2 {
                            assert!(std::time::Instant::now() < deadline, "children were serialized");
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        child_calls.fetch_add(1, Ordering::SeqCst);
                    } else {
                        assert_eq!(child_calls.load(Ordering::SeqCst), 2, "lead advanced before its team");
                        for name in ["join-a", "join-b"] {
                            let receipt = body["messages"].as_array().unwrap().iter().find(|m| m["tool_call_id"] == name).unwrap();
                            let receipt: serde_json::Value = serde_json::from_str(receipt["content"].as_str().unwrap()).unwrap();
                            assert_eq!(receipt["value"]["status"], "completed");
                            assert_eq!(receipt["value"]["result"], "done");
                        }
                    }
                    serde_json::json!({"role":"assistant","content":"done"})
                };
                request.respond(tiny_http::Response::from_string(serde_json::json!({
                    "choices":[{"message":message,"finish_reason":if first || join {"tool_calls"} else {"stop"}}]
                }).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
            }));
        }
        for handler in handlers {
            handler.join().unwrap();
        }
    });
    let config = serde_json::json!({"name":"lead","instructions":"delegate then finish","model":"root-model","endpoint":endpoint,
    "shared_model_budget":{"group":"team-test","limit":10},
    "subagents":[
        {"name":"a","instructions":"finish","model":"a-model","endpoint":endpoint},
        {"name":"b","instructions":"finish","model":"b-model","endpoint":endpoint}
    ]});
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), config.to_string()).unwrap();
    let actor = Actor {
        workspace_id: "local".into(),
        id: "developer".into(),
    };
    let store = Store::default();
    let store_guard = store.clone();
    let tree = Configuration::load(file.path())
        .unwrap()
        .build_temporal_tree(store.clone(), &actor)
        .unwrap();
    let id = tree
        .runtime
        .submit(
            &actor,
            tree.reference.clone(),
            serde_json::json!("work"),
            None,
        )
        .unwrap();
    let inspect_actor = actor.clone();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let env = WorkflowEnvironment::start_local(LocalWorkflowEnvironmentOptions::default())
                .await
                .unwrap();
            let runtime = Runtime::from_current_tokio(Default::default()).unwrap();
            let options = WorkerOptions::new("hudson-team-test")
                .register_workflow::<RunWorkflow>()
                .unwrap()
                .register_activities(RunActivities::new(tree.into_scheduled(), actor))
                .build();
            let mut worker = Worker::new(&runtime, env.client().clone(), options).unwrap();
            let stop = worker.shutdown_handle();
            let handle = env
                .client()
                .start_workflow(
                    RunWorkflow::run,
                    id,
                    WorkflowStartOptions::new("hudson-team-test", format!("test-{id}")).build(),
                )
                .await
                .unwrap();
            let (worker_result, result) = tokio::join!(worker.run(), async {
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(50),
                    handle.get_result(WorkflowGetResultOptions::default()),
                )
                .await;
                stop();
                result
            });
            worker_result.unwrap();
            env.shutdown().await.unwrap();
            assert_eq!(result.unwrap().unwrap().status, RunStatus::Completed);
        });
    assert_eq!(children_done.load(Ordering::SeqCst), 2);
    let children = store.children(&inspect_actor, id).unwrap();
    assert_eq!(children.len(), 2);
    assert!(children
        .iter()
        .all(|child| child.status == RunStatus::Completed));
    server_thread.join().unwrap();
    drop(store_guard);
}

#[test]
#[ignore = "requires local PostgreSQL hudson_harness_test_20260921 and Temporal CLI"]
fn question_survives_worker_restart_with_postgres() {
    use hudson_core::models::WaitReason;
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/chat", server.server_addr());
    let server_thread = std::thread::spawn(move || {
        for index in 0..2 {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(50))
                .unwrap()
                .expect("missing model request");
            let body: serde_json::Value = serde_json::from_reader(request.as_reader()).unwrap();
            let message = if index == 0 {
                serde_json::json!({"role":"assistant","content":null,"tool_calls":[{"id":"question","type":"function","function":{"name":"ask_user","arguments":"{\"prompt\":\"Which city?\"}"}}]})
            } else {
                assert!(body["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["role"] == "tool"
                        && m["tool_call_id"] == "question"
                        && m["content"].as_str().unwrap().contains("Boston")));
                serde_json::json!({"role":"assistant","content":"Boston selected"})
            };
            request.respond(tiny_http::Response::from_string(serde_json::json!({"choices":[{"message":message,"finish_reason":if index == 0 {"tool_calls"} else {"stop"}}]}).to_string())).unwrap();
        }
    });
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), serde_json::json!({"name":"question-agent","instructions":"ask then finish","allow_user_input":true,"endpoint":endpoint}).to_string()).unwrap();
    let namespace = format!("temporal-restart-{}", uuid::Uuid::new_v4());
    let database = std::env::var("HUDSON_TEST_DATABASE")
        .unwrap_or_else(|_| "hudson_harness_test_20260921".into());
    let actor = Actor {
        workspace_id: "local".into(),
        id: "developer".into(),
    };
    let store = Store::postgres_local("/tmp", &database, &namespace).unwrap();
    let store_guard = store.clone();
    let tree = Configuration::load(file.path())
        .unwrap()
        .build_temporal_tree(store.clone(), &actor)
        .unwrap();
    let id = tree
        .runtime
        .submit(
            &actor,
            tree.reference.clone(),
            serde_json::json!("choose a city"),
            Some("restart-test".into()),
        )
        .unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let env = WorkflowEnvironment::start_local(LocalWorkflowEnvironmentOptions::default())
                .await
                .unwrap();
            let runtime = Runtime::from_current_tokio(Default::default()).unwrap();
            let options = WorkerOptions::new("hudson-restart")
                .register_workflow::<RunWorkflow>()
                .unwrap()
                .register_activities(RunActivities::new(tree.into_scheduled(), actor.clone()))
                .build();
            let mut worker = Worker::new(&runtime, env.client().clone(), options).unwrap();
            let stop = worker.shutdown_handle();
            let handle = env
                .client()
                .start_workflow(
                    RunWorkflow::run,
                    id,
                    WorkflowStartOptions::new("hudson-restart", format!("test-{id}")).build(),
                )
                .await
                .unwrap();
            let (worked, question) = tokio::join!(worker.run(), async {
                let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
                loop {
                    let store = store.clone();
                    let actor = actor.clone();
                    let view =
                        tokio::task::spawn_blocking(move || store.inspect(&actor, id).unwrap())
                            .await
                            .unwrap();
                    if let Some(WaitReason::UserInput { question_id, .. }) = view.wait {
                        stop();
                        break question_id;
                    }
                    if tokio::time::Instant::now() >= deadline {
                        stop();
                        panic!("no question");
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            });
            worked.unwrap();
            drop(worker);
            // Reopen PostgreSQL and rebuild every model/tool executor, then apply
            // the answer while no Temporal worker is running.
            let actor2 = actor.clone();
            let tree = tokio::task::spawn_blocking(move || {
                let store = Store::postgres_local("/tmp", &database, &namespace).unwrap();
                let tree = Configuration::load(file.path())
                    .unwrap()
                    .build_temporal_tree(store, &actor2)
                    .unwrap();
                tree.runtime
                    .provide_input(
                        &actor2,
                        id,
                        question,
                        "answer-1",
                        serde_json::json!("Boston"),
                    )
                    .unwrap();
                tree
            })
            .await
            .unwrap();
            let options = WorkerOptions::new("hudson-restart")
                .register_workflow::<RunWorkflow>()
                .unwrap()
                .register_activities(RunActivities::new(tree.into_scheduled(), actor))
                .build();
            let mut worker = Worker::new(&runtime, env.client().clone(), options).unwrap();
            let stop = worker.shutdown_handle();
            let (worked, result) = tokio::join!(worker.run(), async {
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    handle.get_result(WorkflowGetResultOptions::default()),
                )
                .await;
                stop();
                result
            });
            worked.unwrap();
            env.shutdown().await.unwrap();
            assert_eq!(result.unwrap().unwrap().status, RunStatus::Completed);
        });
    server_thread.join().unwrap();
    drop(store_guard);
}

#[test]
#[ignore = "requires local PostgreSQL and starts separate Temporal worker/client processes"]
fn background_returns_without_worker_and_foreground_waits_for_same_run() {
    use std::process::{Child, Command, Stdio};
    struct Process(Child);
    impl Drop for Process {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn output(mut child: Child) -> std::process::Output {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("CLI timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        child.wait_with_output().unwrap()
    }
    let port_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = port_listener.local_addr().unwrap().port();
    drop(port_listener);
    tokio::runtime::Runtime::new().unwrap().block_on(async move {
        let env = WorkflowEnvironment::start_local(LocalWorkflowEnvironmentOptions::builder().port(port).build()).await.unwrap();
        tokio::task::spawn_blocking(move || {
            let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}/chat", server.server_addr());
            let server_thread = std::thread::spawn(move || {
                let mut request = server.recv_timeout(std::time::Duration::from_secs(30)).unwrap().expect("no model call");
                let body: serde_json::Value = serde_json::from_reader(request.as_reader()).unwrap();
                let user = body["messages"].as_array().unwrap().iter().find(|m| m["role"] == "user").unwrap();
                assert_eq!(serde_json::from_str::<serde_json::Value>(user["content"].as_str().unwrap()).unwrap(), serde_json::json!({"task":"finish"}));
                request.respond(tiny_http::Response::from_string(r#"{"choices":[{"message":{"role":"assistant","content":"done"},"finish_reason":"stop"}]}"#)).unwrap();
            });
            let file = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(file.path(), serde_json::json!({"name":"cli-agent","instructions":"finish","endpoint":endpoint,"input_schema":{"type":"object","required":["task"]}}).to_string()).unwrap();
            let input = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(input.path(), r#"{"task":"finish"}"#).unwrap();
            let database = std::env::var("HUDSON_TEST_DATABASE").unwrap_or_else(|_| "hudson_harness_test_20260921".into());
            let namespace = format!("temporal-cli-{}", uuid::Uuid::new_v4());
            let command = || {
                let mut command = Command::new(env!("CARGO_BIN_EXE_hudson-temporal"));
                command.args(["--config", file.path().to_str().unwrap(), "--database", &database, "--namespace", &namespace, "--task-queue", &namespace])
                    .env("TEMPORAL_ADDRESS", format!("127.0.0.1:{port}"))
                    .env("TEMPORAL_NAMESPACE", "default")
                    .stdout(Stdio::piped()).stderr(Stdio::piped());
                command
            };
            let background = output(command().args(["run","--input-file",input.path().to_str().unwrap(),"--request-key","task-1","--background"]).spawn().unwrap());
            assert!(background.status.success(), "{}", String::from_utf8_lossy(&background.stderr));
            let receipt: serde_json::Value = serde_json::from_slice(&background.stdout).unwrap();
            let id = receipt["run_id"].as_str().unwrap();
            let repeated = output(command().args(["run","--input-file",input.path().to_str().unwrap(),"--request-key","task-1","--background"]).spawn().unwrap());
            assert!(repeated.status.success(), "{}", String::from_utf8_lossy(&repeated.stderr));
            assert_eq!(serde_json::from_slice::<serde_json::Value>(&repeated.stdout).unwrap()["run_id"], id);
            let mut foreground = command().args(["run","--resume",id]).spawn().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(500));
            assert!(foreground.try_wait().unwrap().is_none(), "foreground returned before a worker existed");
            let worker = Process(command().arg("worker").spawn().unwrap());
            let foreground = output(foreground);
            assert!(foreground.status.success(), "{}", String::from_utf8_lossy(&foreground.stderr));
            let stdout = String::from_utf8(foreground.stdout).unwrap();
            assert!(stdout.contains(id));
            assert!(stdout.contains("\"status\": \"completed\""));
            server_thread.join().unwrap();
            drop(worker);
            let completed = output(command().args(["run","--resume",id]).spawn().unwrap());
            assert!(completed.status.success(), "{}", String::from_utf8_lossy(&completed.stderr));
            assert!(String::from_utf8(completed.stdout).unwrap().contains("\"status\": \"completed\""));
        }).await.unwrap();
        env.shutdown().await.unwrap();
    });
}
