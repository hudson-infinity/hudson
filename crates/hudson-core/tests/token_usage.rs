#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, ToolRegistry},
    },
    fixtures::{self, FixtureBackend},
    models::*,
    runtime::Runtime,
};
use hudson_harness::{ModelRequest, ModelResponse};
use serde_json::json;
struct Truncated {
    usage: Option<TokenUsage>,
}
impl ModelExecutor for Truncated {
    fn call(&mut self, _: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        self.usage = Some(TokenUsage {
            input_tokens: 12,
            output_tokens: 3,
        });
        Err(ExecutionError::Failed("response truncated".into()))
    }
    fn take_usage(&mut self) -> Option<TokenUsage> {
        self.usage.take()
    }
}
#[test]
fn failed_model_output_still_records_consumption_once() {
    let mut runtime = Runtime::new(
        fixtures::store().unwrap(),
        FixtureBackend,
        Truncated { usage: None },
        ToolRegistry::new(),
    );
    let actor = fixtures::actor();
    let id = runtime
        .submit(
            &actor,
            fixtures::agent_ref(),
            json!({"order_id":"123","action":"lookup"}),
            None,
        )
        .unwrap();
    let run = fixtures::drive(&mut runtime, &actor, id).unwrap();
    assert_eq!(run.status, RunStatus::Failed);
    assert_eq!(run.usage.reported_model_calls, 1);
    assert_eq!(run.usage.reported_input_tokens, 12);
    assert_eq!(run.usage.reported_output_tokens, 3);
    assert_eq!(runtime.tick(&actor, id).unwrap().usage, run.usage);
    let operation = runtime.store.operations(&actor, id).unwrap().remove(0);
    assert_eq!(
        operation.attempts[0].token_usage,
        Some(TokenUsage {
            input_tokens: 12,
            output_tokens: 3
        })
    );
    let old: Usage =
        serde_json::from_value(json!({"model_calls":1,"tool_calls":0,"operations":1})).unwrap();
    assert_eq!(old.reported_model_calls, 0);
}
