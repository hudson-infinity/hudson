#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::tools::ToolRegistry,
    evaluation::{evaluate, validate_cases, Case},
    fixtures::{self, OrderModel},
    models::RunStatus,
    runtime::Runtime,
};
use hudson_harness::AgentLoop;
use serde_json::json;

#[test]
fn evaluation_preserves_approval_and_rejects_invalid_cases_before_dispatch() {
    let actor = fixtures::actor();
    let mut runtime = Runtime::new(
        fixtures::store().unwrap(),
        AgentLoop,
        OrderModel,
        ToolRegistry::new(),
    );
    let case = Case {
        criteria: vec![],
        name: "refund-needs-approval".into(),
        input: json!({"order_id":"123","action":"refund"}),
        expected_schema: json!({}),
    };
    assert!(validate_cases(&[]).is_err());
    assert!(validate_cases(&[case.clone(), case.clone()]).is_err());
    let report = evaluate(&mut runtime, &actor, fixtures::agent_ref(), None, &[case]).unwrap();
    assert!(!report.passed);
    assert_eq!(report.cases[0].run.status, RunStatus::Waiting);
    assert!(report.cases[0].feedback.contains("did not complete"));
    assert!(report.cases[0].run.result.is_none());
    assert!(runtime
        .store
        .operations(&actor, report.cases[0].run.id)
        .unwrap()
        .iter()
        .filter(|op| matches!(
            op.request,
            hudson_core::models::OperationRequest::Tool { .. }
        ))
        .all(|op| op.attempts.is_empty()));
    assert_eq!(report.cases[0].run.usage.tool_calls, 1); // planned tool operation; no dispatch attempt before approval
}

#[test]
fn partial_limits_keep_other_defaults_and_reject_misspellings() {
    let limits: hudson_core::models::Limits =
        serde_json::from_value(json!({"max_model_calls":1})).unwrap();
    assert_eq!(limits.max_model_calls, 1);
    assert_eq!(
        limits.max_harness_steps,
        hudson_core::models::Limits::default().max_harness_steps
    );
    assert!(
        serde_json::from_value::<hudson_core::models::Limits>(json!({"max_model_call":1})).is_err()
    );
}

#[test]
fn held_out_value_criteria_reject_schema_valid_wrong_answers() {
    use hudson_core::{
        adapters::models::ModelExecutor, adapters::tools::ExecutionError, verification::Criterion,
    };
    use hudson_harness::{ModelRequest, ModelResponse};
    struct Fixed;
    impl ModelExecutor for Fixed {
        fn call(&mut self, request: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
            assert!(!serde_json::to_string(request)
                .unwrap()
                .contains("private-expected-answer"));
            Ok(ModelResponse::Final {
                output: json!({"answer":"actual"}),
            })
        }
    }
    let store = hudson_core::storage::Store::default();
    let (mut agent, _) = fixtures::definitions();
    agent.tools.clear();
    agent.input_schema = None;
    agent.output_schema = None;
    let reference = agent.reference();
    store.publish_agent(agent).unwrap();
    let mut runtime = Runtime::new(store, AgentLoop, Fixed, ToolRegistry::new());
    let cases = ["actual", "private-expected-answer"]
        .into_iter()
        .enumerate()
        .map(|(i, expected)| Case {
            name: format!("case-{i}"),
            input: json!("answer the task"),
            expected_schema: json!({"type":"object","required":["answer"]}),
            criteria: vec![Criterion::Equals {
                pointer: "/answer".into(),
                expected: json!(expected),
            }],
        })
        .collect::<Vec<_>>();
    let report = evaluate(&mut runtime, &fixtures::actor(), reference, None, &cases).unwrap();
    assert!(report.cases[0].passed);
    assert!(!report.cases[1].passed);
    assert_eq!(report.cases[1].run.status, RunStatus::Completed);
    assert_eq!(report.cases[1].feedback, "held-out criterion 1 failed");
}
