#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::tools::{ExecutionError, Invocation, ToolExecutor, ToolRegistry},
    fixtures::{self, OrderModel},
    models::{Execution, RunStatus},
    runtime::Runtime,
};
use hudson_harness::AgentLoop;
use serde_json::json;

#[test]
fn application_function_runs_through_the_real_loop_and_runtime() {
    let mut tools = ToolRegistry::new();
    tools
        .register("orders.lookup", |invocation| {
            assert!(!invocation.operation_id.is_nil());
            Ok(json!({"order_id": invocation.arguments["order_id"], "status":"shipped"}))
        })
        .unwrap();
    let mut runtime = Runtime::new(fixtures::store().unwrap(), AgentLoop, OrderModel, tools);
    let id = runtime
        .submit(
            &fixtures::actor(),
            fixtures::agent_ref(),
            json!({"order_id":"123", "action":"lookup"}),
            None,
        )
        .unwrap();
    let result = fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert_eq!(result.status, RunStatus::Completed);
    assert!(result.assessment.unwrap().passed);
}

#[test]
fn registry_preserves_bindings_and_uncertain_effects() {
    let mut registry = ToolRegistry::new();
    registry
        .register("orders.lookup", |_| Ok(json!("original")))
        .unwrap();
    assert!(registry
        .register("orders.lookup", |_| Ok(json!("replacement")))
        .is_err());
    assert!(registry.register("  ", |_| Ok(json!(null))).is_err());
    let (_, mut tools) = fixtures::definitions();
    let mut tool = tools.remove(0);
    tool.execution = Execution::Registered {
        key: "orders.lookup".into(),
    };
    let arguments = json!({});
    fn invoke<'a>(
        tool: &'a hudson_core::models::Tool,
        arguments: &'a serde_json::Value,
    ) -> Invocation<'a> {
        Invocation {
            operation_id: uuid::Uuid::new_v4(),
            tool,
            arguments,
        }
    }
    assert_eq!(
        registry.execute(invoke(&tool, &arguments)).unwrap(),
        json!("original")
    );
    tool.execution = Execution::Http {
        endpoint: "https://example.invalid".into(),
    };
    assert!(matches!(
        registry.execute(invoke(&tool, &arguments)),
        Err(ExecutionError::Failed(_))
    ));
    registry
        .register("panic", |_| panic!("effect may already have happened"))
        .unwrap();
    tool.execution = Execution::Registered {
        key: "panic".into(),
    };
    assert!(matches!(
        registry.execute(invoke(&tool, &arguments)),
        Err(ExecutionError::Unknown(_))
    ));
}
