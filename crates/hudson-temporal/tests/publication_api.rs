use temporalio_sdk::testing::{LocalWorkflowEnvironmentOptions, WorkflowEnvironment};

#[test]
#[ignore = "requires built server/credential binaries, PostgreSQL and a local Temporal server"]
fn customer_publishes_tools_and_agents_then_restarts_api_and_worker() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let env = WorkflowEnvironment::start_local(
                LocalWorkflowEnvironmentOptions::builder()
                    .port(port)
                    .build(),
            )
            .await
            .unwrap();
            let result = tokio::task::spawn_blocking(move || {
                std::process::Command::new("python3")
                    .arg("scripts/smoke_publication.py")
                    .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                    .env(
                        "HUDSON_TEST_BIN_DIR",
                        std::path::Path::new(env!("CARGO_BIN_EXE_hudson-temporal"))
                            .parent()
                            .unwrap(),
                    )
                    .env(
                        "HUDSON_TEST_DATABASE",
                        std::env::var("HUDSON_TEST_DATABASE")
                            .unwrap_or_else(|_| "hudson_harness_test_20260921".into()),
                    )
                    .env("TEMPORAL_ADDRESS", format!("127.0.0.1:{port}"))
                    .env("TEMPORAL_NAMESPACE", "default")
                    .output()
                    .unwrap()
            })
            .await
            .unwrap();
            env.shutdown().await.unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            println!("{}", String::from_utf8_lossy(&result.stdout));
        });
}
