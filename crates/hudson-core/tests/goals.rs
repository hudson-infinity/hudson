#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, ToolRegistry},
    },
    fixtures,
    models::*,
    runtime::Runtime,
    storage::Store,
};
use hudson_harness::{AgentLoop, ModelRequest, ModelResponse};
use serde_json::json;
struct Answer;
impl ModelExecutor for Answer {
    fn call(&mut self, request: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        assert!(request.instructions.contains("Return the answer"));
        Ok(ModelResponse::Final {
            output: json!({"answer":42}),
        })
    }
}
#[test]
fn goal_contract_is_persisted_and_enforced() {
    for expected in [42, 43] {
        let store = Store::default();
        let (mut agent, _) = fixtures::definitions();
        agent.tools.clear();
        agent.input_schema = None;
        agent.output_schema = None;
        agent.limits.max_model_calls = 2;
        let reference = agent.reference();
        store.publish_agent(agent).unwrap();
        let goal = Goal {
            criteria: vec![hudson_core::verification::Criterion::Equals {
                pointer: "/answer".into(),
                expected: json!(expected),
            }],
            objective: "Return the answer".into(),
            success_schema: json!({"type":"object","properties":{"answer":{"type":"integer"}},"required":["answer"]}),
        };
        let mut runtime = Runtime::new(store, AgentLoop, Answer, ToolRegistry::new());
        let id = runtime
            .submit_with_goal(
                &fixtures::actor(),
                reference.clone(),
                json!("task"),
                Some("key".into()),
                Some(goal.clone()),
            )
            .unwrap();
        assert_eq!(
            runtime.store.inspect(&fixtures::actor(), id).unwrap().goal,
            Some(goal.clone())
        );
        let mut changed = goal;
        changed.objective = "Changed objective".into();
        assert!(runtime
            .submit_with_goal(
                &fixtures::actor(),
                reference,
                json!("task"),
                Some("key".into()),
                Some(changed)
            )
            .is_err());
        let view = fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
        assert_eq!(view.status == RunStatus::Completed, expected == 42);
        assert_eq!(view.assessment.unwrap().passed, expected == 42);
    }
}
