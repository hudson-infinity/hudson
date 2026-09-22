#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, Invocation, ToolExecutor},
    },
    fixtures::{self, FixtureBackend, FixtureTools, OrderModel},
    models::*,
    runtime::Runtime,
    security::Policy,
    Error,
};
use hudson_harness::{
    Action, Backend, Checkpoint, Config, HarnessError, Input, ModelRequest, ModelResponse,
    ToolCall, Transition,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

fn input() -> Value {
    json!({"order_id":"123","action":"lookup"})
}
fn setup() -> (fixtures::DemoRuntime, Arc<Mutex<Vec<uuid::Uuid>>>) {
    let tools = FixtureTools::default();
    let calls = tools.executed.clone();
    (
        Runtime::new(
            fixtures::store().unwrap(),
            FixtureBackend,
            OrderModel,
            tools,
        ),
        calls,
    )
}
fn policy(allowed: bool, require_approval: bool) -> Policy {
    Policy {
        actors: if allowed {
            ["developer".into()].into()
        } else {
            Default::default()
        },
        approvers: ["developer".into()].into(),
        require_approval,
    }
}
fn waiting_refund(runtime: &mut fixtures::DemoRuntime) -> (uuid::Uuid, uuid::Uuid) {
    let id = runtime
        .submit(
            &fixtures::actor(),
            fixtures::agent_ref(),
            json!({"order_id":"123","action":"refund"}),
            None,
        )
        .unwrap();
    let view = fixtures::drive(runtime, &fixtures::actor(), id).unwrap();
    let Some(WaitReason::Approval { operation_id }) = view.wait else {
        panic!("expected approval")
    };
    (id, operation_id)
}

#[test]
fn tool_roundtrip_records_correlated_result_and_passed_assessment() {
    let (mut runtime, calls) = setup();
    let id = runtime
        .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
        .unwrap();
    let view = fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert_eq!(view.status, RunStatus::Completed);
    assert!(view.assessment.unwrap().passed);
    assert_eq!(calls.lock().unwrap().len(), 1);
    let ops = runtime.store.operations(&fixtures::actor(), id).unwrap();
    assert_eq!(ops.len(), 4); // Model, tool, model, deterministic verification.
    assert!(ops.iter().all(|o| o.status == OperationStatus::Succeeded));
    let events = runtime.store.events(&fixtures::actor(), id, 0).unwrap();
    assert_eq!(
        events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
        (1..=events.len() as u64).collect::<Vec<_>>()
    );
    let cursor = events[2].sequence;
    assert!(runtime
        .store
        .events(&fixtures::actor(), id, cursor)
        .unwrap()
        .iter()
        .all(|e| e.sequence > cursor));
    for op in ops {
        let encoded = serde_json::to_vec(&op).unwrap();
        assert_eq!(op, serde_json::from_slice::<Operation>(&encoded).unwrap());
    }
    // Terminal calls never repeat external work.
    runtime.tick(&fixtures::actor(), id).unwrap();
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn current_policy_denial_never_reaches_executor() {
    let (mut runtime, calls) = setup();
    runtime
        .store
        .set_policy("demo", "lookup_order", policy(false, false))
        .unwrap();
    let id = runtime
        .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
        .unwrap();
    fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert!(calls.lock().unwrap().is_empty());
    let denied = runtime
        .store
        .operations(&fixtures::actor(), id)
        .unwrap()
        .into_iter()
        .find(|o| o.status == OperationStatus::Denied)
        .unwrap();
    assert!(denied.attempts.is_empty());
}

#[test]
fn approval_wait_resumes_in_a_new_driver_and_executes_once() {
    let (mut runtime, calls) = setup();
    let (id, op) = waiting_refund(&mut runtime);
    assert!(calls.lock().unwrap().is_empty());
    let store = runtime.store.clone();
    drop(runtime);
    let mut resumed = Runtime::new(
        store,
        FixtureBackend,
        OrderModel,
        FixtureTools {
            executed: calls.clone(),
            uncertain: false,
        },
    );
    resumed
        .approve(&fixtures::actor(), op, true, now(), now() + 60_000)
        .unwrap();
    assert_eq!(
        fixtures::drive(&mut resumed, &fixtures::actor(), id)
            .unwrap()
            .status,
        RunStatus::Completed
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(
        resumed
            .store
            .operations(&fixtures::actor(), id)
            .unwrap()
            .into_iter()
            .find(|o| o.meta.id == op)
            .unwrap()
            .approval
            .unwrap()
            .decisions
            .len(),
        1
    );
}

#[test]
fn expired_approval_and_revoked_access_prevent_dispatch() {
    let (mut runtime, calls) = setup();
    let (id, op) = waiting_refund(&mut runtime);
    runtime
        .approve(&fixtures::actor(), op, true, now() - 1000, now() - 1)
        .unwrap();
    assert_eq!(
        runtime.tick(&fixtures::actor(), id).unwrap().status,
        RunStatus::Waiting
    );
    assert!(calls.lock().unwrap().is_empty());
    runtime
        .approve(&fixtures::actor(), op, true, now(), now() + 60_000)
        .unwrap();
    runtime
        .store
        .set_policy("demo", "refund_order", policy(false, true))
        .unwrap();
    fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn unapproved_principal_cannot_approve_and_denial_is_a_result() {
    let (mut runtime, calls) = setup();
    let (id, op) = waiting_refund(&mut runtime);
    let outsider = Actor {
        id: "outsider".into(),
        ..fixtures::actor()
    };
    assert!(matches!(
        runtime.approve(&outsider, op, true, now(), now() + 1000),
        Err(Error::Denied)
    ));
    runtime
        .approve(&fixtures::actor(), op, false, now(), now() + 1000)
        .unwrap();
    fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn uncertain_write_is_not_replayed_and_a_receipt_can_resume_it() {
    let store = fixtures::store().unwrap();
    let tools = FixtureTools {
        uncertain: true,
        ..Default::default()
    };
    let calls = tools.executed.clone();
    let mut runtime = Runtime::new(store, FixtureBackend, OrderModel, tools);
    let (id, op) = waiting_refund(&mut runtime);
    runtime
        .approve(&fixtures::actor(), op, true, now(), now() + 60_000)
        .unwrap();
    let view = fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert!(matches!(view.wait, Some(WaitReason::Reconciliation { .. })));
    for _ in 0..3 {
        runtime.tick(&fixtures::actor(), id).unwrap();
    }
    assert_eq!(calls.lock().unwrap().len(), 1);
    runtime
        .record_reconciled_tool_result(
            &fixtures::actor(),
            op,
            json!({"status":"refunded"}),
            "trusted-destination-receipt",
        )
        .unwrap();
    assert_eq!(
        fixtures::drive(&mut runtime, &fixtures::actor(), id)
            .unwrap()
            .status,
        RunStatus::Completed
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn cancelling_approval_wait_stops_dispatch() {
    let (mut runtime, calls) = setup();
    let (id, op) = waiting_refund(&mut runtime);
    runtime.cancel(&fixtures::actor(), id).unwrap();
    assert_eq!(
        runtime.tick(&fixtures::actor(), id).unwrap().status,
        RunStatus::Cancelled
    );
    assert!(runtime
        .approve(&fixtures::actor(), op, true, now(), now() + 1000)
        .is_err());
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn cancellation_preserves_unknown_outcome_until_reconciled() {
    let tools = FixtureTools {
        uncertain: true,
        ..Default::default()
    };
    let mut runtime = Runtime::new(
        fixtures::store().unwrap(),
        FixtureBackend,
        OrderModel,
        tools,
    );
    let (id, op) = waiting_refund(&mut runtime);
    runtime
        .approve(&fixtures::actor(), op, true, now(), now() + 60_000)
        .unwrap();
    fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    runtime.cancel(&fixtures::actor(), id).unwrap();
    assert_eq!(
        runtime.tick(&fixtures::actor(), id).unwrap().status,
        RunStatus::Cancelling
    );
    runtime
        .record_reconciled_tool_result(
            &fixtures::actor(),
            op,
            json!({"status":"refunded"}),
            "receipt",
        )
        .unwrap();
    assert_eq!(
        runtime.tick(&fixtures::actor(), id).unwrap().status,
        RunStatus::Cancelled
    );
}

#[test]
fn submissions_are_deduplicated_and_changed_input_conflicts() {
    let (runtime, _) = setup();
    let id = runtime
        .submit(
            &fixtures::actor(),
            fixtures::agent_ref(),
            input(),
            Some("request-1".into()),
        )
        .unwrap();
    assert_eq!(
        runtime
            .submit(
                &fixtures::actor(),
                fixtures::agent_ref(),
                input(),
                Some("request-1".into())
            )
            .unwrap(),
        id
    );
    assert!(matches!(
        runtime.submit(
            &fixtures::actor(),
            fixtures::agent_ref(),
            json!({"order_id":"other","action":"lookup"}),
            Some("request-1".into())
        ),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn concurrent_submission_shares_one_run() {
    let store = fixtures::store().unwrap();
    let ids = std::thread::scope(|s| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let store = store.clone();
                s.spawn(move || {
                    Runtime::new(store, FixtureBackend, OrderModel, FixtureTools::default())
                        .submit(
                            &fixtures::actor(),
                            fixtures::agent_ref(),
                            input(),
                            Some("same".into()),
                        )
                        .unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(ids.iter().all(|id| *id == ids[0]));
}

#[test]
fn definitions_are_immutable_and_workspace_scoped() {
    let store = fixtures::store().unwrap();
    let (mut agent, tools) = fixtures::definitions();
    assert!(matches!(
        store.publish_agent(agent.clone()),
        Err(Error::Conflict(_))
    ));
    assert!(matches!(
        store.publish_tool(tools[0].clone()),
        Err(Error::Conflict(_))
    ));
    agent.workspace_id = "other".into();
    assert!(matches!(store.publish_agent(agent), Err(Error::NotFound)));
    let runtime = Runtime::new(store, FixtureBackend, OrderModel, FixtureTools::default());
    let id = runtime
        .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
        .unwrap();
    let foreign = Actor {
        workspace_id: "other".into(),
        ..fixtures::actor()
    };
    assert!(matches!(
        runtime.store.inspect(&foreign, id),
        Err(Error::NotFound)
    ));
    assert!(runtime.store.events(&foreign, id, 0).is_err());
    assert!(runtime.store.operations(&foreign, id).is_err());
}

struct FixedModel(ModelResponse);
impl ModelExecutor for FixedModel {
    fn call(&mut self, _: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        Ok(self.0.clone())
    }
}
#[test]
fn unknown_tools_bad_arguments_and_duplicate_call_ids_never_dispatch() {
    for calls in [
        vec![ToolCall {
            provider_metadata: serde_json::Value::Null,
            call_id: "a".into(),
            name: "erase_orders".into(),
            arguments: json!({}),
        }],
        vec![ToolCall {
            provider_metadata: serde_json::Value::Null,
            call_id: "a".into(),
            name: "lookup_order".into(),
            arguments: json!({"wrong":1}),
        }],
        vec![
            ToolCall {
                provider_metadata: serde_json::Value::Null,
                call_id: "same".into(),
                name: "lookup_order".into(),
                arguments: json!({"order_id":"1"})
            };
            2
        ],
    ] {
        let tools = FixtureTools::default();
        let executions = tools.executed.clone();
        let mut runtime = Runtime::new(
            fixtures::store().unwrap(),
            FixtureBackend,
            FixedModel(ModelResponse::ToolCalls { calls }),
            tools,
        );
        let id = runtime
            .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
            .unwrap();
        assert_eq!(
            fixtures::drive(&mut runtime, &fixtures::actor(), id)
                .unwrap()
                .status,
            RunStatus::Failed
        );
        assert!(executions.lock().unwrap().is_empty());
    }
}

#[test]
fn required_output_schema_failure_does_not_report_completion() {
    let mut runtime = Runtime::new(
        fixtures::store().unwrap(),
        FixtureBackend,
        FixedModel(ModelResponse::Final {
            output: json!({"wrong":true}),
        }),
        FixtureTools::default(),
    );
    let id = runtime
        .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
        .unwrap();
    let view = fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert_eq!(view.status, RunStatus::Failed);
    assert!(!view.assessment.unwrap().passed);
    assert!(view.result.is_none());
}

#[test]
fn budget_reservation_rejects_whole_batch_before_dispatch() {
    let store = fixtures::store().unwrap();
    let (mut agent, _) = fixtures::definitions();
    agent.version = 2;
    agent.limits.max_operations = 2;
    store.publish_agent(agent.clone()).unwrap();
    let calls = (1..=2)
        .map(|n| ToolCall {
            provider_metadata: serde_json::Value::Null,
            call_id: n.to_string(),
            name: "lookup_order".into(),
            arguments: json!({"order_id":"123"}),
        })
        .collect();
    let tools = FixtureTools::default();
    let executed = tools.executed.clone();
    let mut runtime = Runtime::new(
        store,
        FixtureBackend,
        FixedModel(ModelResponse::ToolCalls { calls }),
        tools,
    );
    let id = runtime
        .submit(&fixtures::actor(), agent.reference(), input(), None)
        .unwrap();
    let view = fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert_eq!(view.status, RunStatus::Failed);
    assert_eq!(
        runtime
            .store
            .operations(&fixtures::actor(), id)
            .unwrap()
            .len(),
        1
    );
    assert!(executed.lock().unwrap().is_empty());
}

#[test]
fn unsupported_sandbox_never_falls_back_to_registered_executor() {
    let store = fixtures::store().unwrap();
    let (mut agent, mut tools) = fixtures::definitions();
    let tool = &mut tools[0];
    tool.version = 2;
    tool.execution = Execution::Sandbox {
        package: "unavailable".into(),
    };
    store.publish_tool(tool.clone()).unwrap();
    agent.version = 2;
    agent.tools[0].tool_ref = tool.reference();
    store.publish_agent(agent.clone()).unwrap();
    let executor = FixtureTools::default();
    let calls = executor.executed.clone();
    let mut runtime = Runtime::new(store, FixtureBackend, OrderModel, executor);
    let id = runtime
        .submit(&fixtures::actor(), agent.reference(), input(), None)
        .unwrap();
    fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn schemas_cannot_fetch_external_references() {
    assert!(hudson_core::definitions::validate_schema(
        &json!({"$ref":"https://example.com/schema"})
    )
    .is_err());
    assert!(
        hudson_core::definitions::validate_schema(&json!({"$ref":"file:///etc/passwd"})).is_err()
    );
    assert!(hudson_core::definitions::validate_schema(
        &json!({"$defs":{"text":{"type":"string"}},"$ref":"#/$defs/text"})
    )
    .is_ok());
}

#[test]
fn active_effect_is_not_dispatched_twice_and_cancellation_keeps_its_result() {
    use std::sync::mpsc;
    struct BlockingTools {
        started: mpsc::SyncSender<()>,
        finish: mpsc::Receiver<()>,
    }
    impl ToolExecutor for BlockingTools {
        fn execute(&mut self, _: Invocation<'_>) -> Result<Value, ExecutionError> {
            self.started.send(()).unwrap();
            self.finish.recv().unwrap();
            Ok(json!({"status":"shipped"}))
        }
    }
    let store = fixtures::store().unwrap();
    let (started_tx, started_rx) = mpsc::sync_channel(0);
    let (finish_tx, finish_rx) = mpsc::sync_channel(0);
    let mut first = Runtime::new(
        store.clone(),
        FixtureBackend,
        FixedModel(ModelResponse::ToolCalls {
            calls: (1..=2)
                .map(|index| ToolCall {
                    provider_metadata: serde_json::Value::Null,
                    call_id: index.to_string(),
                    name: "lookup_order".into(),
                    arguments: json!({"order_id":"123"}),
                })
                .collect(),
        }),
        BlockingTools {
            started: started_tx,
            finish: finish_rx,
        },
    );
    let id = first
        .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
        .unwrap();
    for _ in 0..3 {
        first.tick(&fixtures::actor(), id).unwrap();
    }
    let handle = std::thread::spawn(move || first.tick(&fixtures::actor(), id).unwrap());
    started_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let executor = FixtureTools::default();
    let calls = executor.executed.clone();
    let mut second = Runtime::new(store, FixtureBackend, OrderModel, executor);
    second.tick(&fixtures::actor(), id).unwrap();
    assert!(calls.lock().unwrap().is_empty());
    second.cancel(&fixtures::actor(), id).unwrap();
    assert_eq!(
        second.store.inspect(&fixtures::actor(), id).unwrap().status,
        RunStatus::Cancelling
    );
    finish_tx.send(()).unwrap();
    handle.join().unwrap();
    assert_eq!(
        second.tick(&fixtures::actor(), id).unwrap().status,
        RunStatus::Cancelled
    );
    let ops = second.store.operations(&fixtures::actor(), id).unwrap();
    assert!(ops
        .iter()
        .any(|o| matches!(o.request, OperationRequest::Tool { .. }) && o.result.is_some()));
}

struct WaitBackend;

#[test]
fn uncertain_batch_member_blocks_later_dispatch_on_repeated_ticks() {
    let executor = FixtureTools {
        uncertain: true,
        ..Default::default()
    };
    let calls = executor.executed.clone();
    let mut runtime = Runtime::new(
        fixtures::store().unwrap(),
        FixtureBackend,
        FixedModel(ModelResponse::ToolCalls {
            calls: (1..=2)
                .map(|index| ToolCall {
                    provider_metadata: serde_json::Value::Null,
                    call_id: index.to_string(),
                    name: "lookup_order".into(),
                    arguments: json!({"order_id":"123"}),
                })
                .collect(),
        }),
        executor,
    );
    let id = runtime
        .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
        .unwrap();
    fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
    for _ in 0..4 {
        runtime.tick(&fixtures::actor(), id).unwrap();
    }
    assert_eq!(calls.lock().unwrap().len(), 1);
    let operations = runtime.store.operations(&fixtures::actor(), id).unwrap();
    assert_eq!(
        operations
            .iter()
            .filter(|o| o.status == OperationStatus::Unknown)
            .count(),
        1
    );
    assert_eq!(
        operations
            .iter()
            .filter(|o| o.status == OperationStatus::Pending)
            .count(),
        1
    );
}

#[test]
fn harness_step_limit_also_bounds_non_model_wait_cycles() {
    let store = fixtures::store().unwrap();
    let (mut agent, _) = fixtures::definitions();
    agent.version = 2;
    agent.limits.max_harness_steps = 1;
    store.publish_agent(agent.clone()).unwrap();
    let mut runtime = Runtime::new(store, WaitBackend, OrderModel, FixtureTools::default());
    let id = runtime
        .submit(&fixtures::actor(), agent.reference(), input(), None)
        .unwrap();
    runtime.tick(&fixtures::actor(), id).unwrap();
    runtime
        .provide_input(&fixtures::actor(), id, 1, "one", json!("reply"))
        .unwrap();
    assert_eq!(
        runtime.tick(&fixtures::actor(), id).unwrap().status,
        RunStatus::Failed
    );
}

#[test]
fn publication_of_new_version_does_not_change_a_submitted_run() {
    let (mut runtime, _) = setup();
    let id = runtime
        .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
        .unwrap();
    let (mut later, _) = fixtures::definitions();
    later.version = 2;
    later.output_schema = Some(json!(false));
    runtime.store.publish_agent(later.clone()).unwrap();
    assert_eq!(
        fixtures::drive(&mut runtime, &fixtures::actor(), id)
            .unwrap()
            .status,
        RunStatus::Completed
    );
    let new_id = runtime
        .submit(&fixtures::actor(), later.reference(), input(), None)
        .unwrap();
    assert_eq!(
        fixtures::drive(&mut runtime, &fixtures::actor(), new_id)
            .unwrap()
            .status,
        RunStatus::Failed
    );
}
impl Backend for WaitBackend {
    fn name(&self) -> &str {
        "wait"
    }
    fn version(&self) -> u32 {
        1
    }
    fn advance(
        &self,
        _: &Config,
        state: &Checkpoint,
        _: Input,
    ) -> Result<Transition, HarnessError> {
        Ok(Transition {
            checkpoint: Checkpoint {
                step: state.step + 1,
                ..state.clone()
            },
            action: Action::WaitForInput {
                prompt: "More detail?".into(),
            },
        })
    }
}
#[test]
fn clarification_delivery_is_deduplicated() {
    let mut runtime = Runtime::new(
        fixtures::store().unwrap(),
        WaitBackend,
        OrderModel,
        FixtureTools::default(),
    );
    let id = runtime
        .submit(&fixtures::actor(), fixtures::agent_ref(), input(), None)
        .unwrap();
    assert_eq!(
        runtime.tick(&fixtures::actor(), id).unwrap().status,
        RunStatus::Waiting
    );
    runtime
        .provide_input(&fixtures::actor(), id, 1, "input-1", json!("details"))
        .unwrap();
    runtime
        .provide_input(&fixtures::actor(), id, 1, "input-1", json!("details"))
        .unwrap();
    assert!(runtime
        .provide_input(&fixtures::actor(), id, 1, "input-1", json!("changed"))
        .is_err());
    assert_eq!(
        runtime
            .store
            .events(&fixtures::actor(), id, 0)
            .unwrap()
            .iter()
            .filter(|e| e.event_type == EventType::InputReceived)
            .count(),
        1
    );
    let second = runtime.tick(&fixtures::actor(), id).unwrap();
    assert!(matches!(
        second.wait,
        Some(WaitReason::UserInput { question_id: 2, .. })
    ));
    // A retry of the accepted answer is harmless, but a delayed new delivery
    // for the previous question must not answer this one.
    runtime
        .provide_input(&fixtures::actor(), id, 1, "input-1", json!("details"))
        .unwrap();
    assert!(runtime
        .provide_input(
            &fixtures::actor(),
            id,
            1,
            "late-delivery",
            json!("old answer")
        )
        .is_err());
    assert!(runtime
        .provide_input(&fixtures::actor(), id, 2, "input-1", json!("details"))
        .is_err());
    assert_eq!(
        runtime
            .store
            .inspect(&fixtures::actor(), id)
            .unwrap()
            .status,
        RunStatus::Waiting
    );
    runtime
        .provide_input(&fixtures::actor(), id, 2, "input-2", json!("new answer"))
        .unwrap();
    assert_eq!(
        runtime
            .store
            .events(&fixtures::actor(), id, 0)
            .unwrap()
            .iter()
            .filter(|e| e.event_type == EventType::InputReceived)
            .count(),
        2
    );
}

#[test]
fn write_with_invalid_returned_receipt_stays_uncertain_without_replay() {
    for oversized in [false, true] {
        let store = hudson_core::storage::Store::default();
        let (agent, mut tools) = fixtures::definitions();
        if !oversized {
            tools[1].output_schema = Some(json!({"type":"object","required":["required_receipt"]}));
        }
        for tool in tools {
            store
                .set_policy("demo", &tool.policy_ref, policy(true, false))
                .unwrap();
            store.publish_tool(tool).unwrap();
        }
        store.publish_agent(agent).unwrap();
        struct Written {
            count: Arc<Mutex<u32>>,
            oversized: bool,
        }
        impl ToolExecutor for Written {
            fn execute(&mut self, _: Invocation<'_>) -> std::result::Result<Value, ExecutionError> {
                *self.count.lock().unwrap() += 1;
                Ok(if self.oversized {
                    json!({"receipt":"x".repeat(70000)})
                } else {
                    json!({"saved":true})
                })
            }
        }
        let count = Arc::new(Mutex::new(0));
        let mut runtime = Runtime::new(
            store,
            FixtureBackend,
            OrderModel,
            Written {
                count: count.clone(),
                oversized,
            },
        );
        let id = runtime
            .submit(
                &fixtures::actor(),
                fixtures::agent_ref(),
                json!({"order_id":"123","action":"refund"}),
                None,
            )
            .unwrap();
        let view = fixtures::drive(&mut runtime, &fixtures::actor(), id).unwrap();
        assert!(matches!(view.wait, Some(WaitReason::Reconciliation { .. })));
        for _ in 0..3 {
            runtime.tick(&fixtures::actor(), id).unwrap();
        }
        assert_eq!(*count.lock().unwrap(), 1);
        assert!(runtime
            .store
            .operations(&fixtures::actor(), id)
            .unwrap()
            .iter()
            .any(|op| op.status == OperationStatus::Unknown));
    }
}
