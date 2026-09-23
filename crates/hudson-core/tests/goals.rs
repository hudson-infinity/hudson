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

#[test]
fn completion_requires_recorded_tool_check_and_repairs_unsupported_claim() {
    use hudson_core::verification::Criterion;
    struct CheckModel(usize);
    impl ModelExecutor for CheckModel {
        fn call(&mut self, request: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
            self.0 += 1;
            if self.0 == 2 {
                assert!(serde_json::to_string(request)
                    .unwrap()
                    .contains("success criterion 1 failed"));
                return Ok(ModelResponse::ToolCalls {
                    calls: vec![hudson_harness::ToolCall {
                        provider_metadata: serde_json::Value::Null,
                        call_id: "verify-shipment".into(),
                        name: "lookup_order".into(),
                        arguments: json!({"order_id":"123"}),
                    }],
                });
            }
            Ok(ModelResponse::Final {
                output: json!({"message":"shipped"}),
            })
        }
    }
    let actor = fixtures::actor();
    let mut runtime = Runtime::new(
        fixtures::store().unwrap(),
        AgentLoop,
        CheckModel(0),
        fixtures::FixtureTools::default(),
    );
    let criterion = Criterion::ToolResultEquals {
        require_latest_tool: true,
        tool_name: "lookup_order".into(),
        arguments: Some(json!({"order_id":"123"})),
        pointer: "/status".into(),
        expected: json!("shipped"),
    };
    assert!(!criterion.matches(&json!({"status":"shipped"})));
    let id = runtime
        .submit_with_goal(
            &actor,
            fixtures::agent_ref(),
            json!({"order_id":"123","action":"lookup"}),
            None,
            Some(Goal {
                objective: "Check shipment".into(),
                success_schema: json!({}),
                criteria: vec![criterion.clone()],
            }),
        )
        .unwrap();
    let view = fixtures::drive(&mut runtime, &actor, id).unwrap();
    assert_eq!(view.status, RunStatus::Completed);
    assert_eq!(view.usage.model_calls, 3);
    assert_eq!(view.assessment.unwrap().evidence.len(), 1);
    let mut operations = runtime.store.operations(&actor, id).unwrap();
    assert!(criterion.matches_with_operations(&json!(null), &operations));
    let mut latest = operations
        .iter()
        .find(|o| matches!(o.request, OperationRequest::Tool { .. }))
        .unwrap()
        .clone();
    latest.step_index += 10;
    let mut unrelated = latest.clone();
    if let OperationRequest::Tool { call, .. } = &mut unrelated.request {
        call.name = "edit_file".into();
    }
    operations.push(unrelated);
    assert!(
        !criterion.matches_with_operations(&json!(null), &operations),
        "verification must follow subsequent tool work when freshness is required"
    );
    operations.pop();
    latest.status = OperationStatus::Failed;
    latest.result = None;
    operations.push(latest);
    assert!(
        !criterion.matches_with_operations(&json!(null), &operations),
        "an older passing check cannot hide a newer failure"
    );
}
