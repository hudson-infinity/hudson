#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, Invocation, ToolExecutor, ToolRegistry},
    },
    fixtures,
    runtime::Runtime,
    subagents,
};
use hudson_harness::{AgentLoop, ModelRequest, ModelResponse};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
struct Model(Arc<AtomicUsize>);
impl ModelExecutor for Model {
    fn call(&mut self, request: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        assert!(request.tools.is_empty());
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ModelResponse::Final {
            output: json!("child result"),
        })
    }
}
#[test]
fn child_runs_once_for_the_same_parent_operation() {
    let store = hudson_core::storage::Store::default();
    let (mut agent, _) = fixtures::definitions();
    agent.tools.clear();
    agent.input_schema = None;
    agent.output_schema = None;
    let reference = agent.reference();
    store.publish_agent(agent).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let child = Runtime::new(
        store.clone(),
        AgentLoop,
        Model(calls.clone()),
        ToolRegistry::new(),
    );
    let mut registry = ToolRegistry::new();
    let tool = subagents::register(
        &mut registry,
        child,
        fixtures::actor(),
        reference,
        "delegate",
        "Delegate analysis",
        "delegate-policy",
    )
    .unwrap();
    let operation_id = uuid::Uuid::new_v4();
    let arguments = json!({"task":"analyze"});
    let mut outputs = Vec::new();
    for _ in 0..2 {
        outputs.push(
            registry
                .execute(Invocation {
                    operation_id,
                    tool: &tool,
                    arguments: &arguments,
                })
                .unwrap(),
        );
    }
    assert_eq!(outputs[0]["child_run"], outputs[1]["child_run"]);
    assert_eq!(outputs[0]["status"], "completed");
    assert_eq!(outputs[0]["result"], "child result");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(registry
        .execute(Invocation {
            operation_id,
            tool: &tool,
            arguments: &json!({"task":"changed"})
        })
        .is_err());
}
