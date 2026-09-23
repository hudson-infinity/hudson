#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, ToolRegistry},
    },
    coordination::CoordinationPolicy,
    fixtures,
    models::*,
    runtime::Runtime,
    security::Policy,
    storage::Store,
    subagents,
};
use hudson_harness::{AgentLoop, ModelRequest, ModelResponse, ToolCall};
use serde_json::{json, Value};
use std::sync::{Arc, Condvar, Mutex};
use uuid::Uuid;
#[derive(Default)]
struct Gate {
    arrivals: Mutex<usize>,
    changed: Condvar,
}
impl Gate {
    fn wait(&self) {
        let mut arrivals = self.arrivals.lock().unwrap();
        *arrivals += 1;
        self.changed.notify_all();
        let (_guard, timeout) = self
            .changed
            .wait_timeout_while(arrivals, std::time::Duration::from_secs(10), |arrivals| {
                *arrivals < 2
            })
            .unwrap();
        assert!(!timeout.timed_out(), "independent children were serialized");
    }
}
struct Parent {
    calls: Vec<ToolCall>,
}
impl ModelExecutor for Parent {
    fn call(&mut self, _: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        if self.calls.is_empty() {
            return Ok(ModelResponse::Final {
                output: json!("finished"),
            });
        }
        Ok(ModelResponse::ToolCalls {
            calls: std::mem::take(&mut self.calls),
        })
    }
}
struct Child {
    barrier: Option<Arc<Gate>>,
    instructions: Arc<Mutex<Vec<String>>>,
    fail: bool,
}
impl ModelExecutor for Child {
    fn call(&mut self, request: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        self.instructions
            .lock()
            .unwrap()
            .push(request.instructions.clone());
        if self.fail {
            return Err(ExecutionError::Failed("child failed".into()));
        }
        if let Some(barrier) = &self.barrier {
            barrier.wait();
        }
        let invalid = request.messages.first().is_some_and(|message| {
            message.content
                == vec![hudson_harness::Content::Json {
                    value: json!("same"),
                }]
        });
        Ok(ModelResponse::Final {
            output: if invalid {
                json!("invalid")
            } else {
                json!({"value":"source-evidence"})
            },
        })
    }
}
fn child(
    store: Store,
    barrier: Option<Arc<Gate>>,
    instructions: Arc<Mutex<Vec<String>>>,
    fail: bool,
) -> Runtime<AgentLoop, Child, ToolRegistry> {
    Runtime::new(
        store,
        AgentLoop,
        Child {
            barrier,
            instructions,
            fail,
        },
        ToolRegistry::new(),
    )
}
fn calls(tasks: Vec<Value>) -> Vec<ToolCall> {
    tasks
        .into_iter()
        .enumerate()
        .map(|(i, arguments)| ToolCall {
            call_id: format!("delegate-{i}"),
            name: "delegate_worker".into(),
            arguments,
            provider_metadata: Value::Null,
        })
        .collect()
}
fn prepare(
    store: Store,
    tasks: Vec<Value>,
    policy: CoordinationPolicy,
) -> (Runtime<AgentLoop, Parent, ToolRegistry>, Uuid, VersionRef) {
    let (mut worker, _) = fixtures::definitions();
    worker.id = "worker".into();
    worker.tools.clear();
    worker.input_schema = None;
    worker.output_schema = Some(json!({"type":"object","required":["value"]}));
    let worker_ref = worker.reference();
    store.publish_agent(worker).unwrap();
    let runtime = child(store.clone(), None, Default::default(), false);
    let mut registry = ToolRegistry::new();
    let tools = subagents::register_deferred_with_join(
        &mut registry,
        Arc::new(Mutex::new(runtime)),
        fixtures::actor(),
        worker_ref.clone(),
        "delegate_worker",
        "Delegate work",
        "delegate",
    )
    .unwrap();
    let (mut lead, _) = fixtures::definitions();
    lead.id = "lead".into();
    lead.input_schema = None;
    lead.output_schema = None;
    lead.limits.max_batch_size = 8;
    lead.tools = tools
        .iter()
        .map(|tool| AgentTool {
            tool_ref: tool.reference(),
            alias: tool.name.clone(),
        })
        .collect();
    for tool in tools {
        store.publish_tool(tool).unwrap();
    }
    store
        .ensure_policy(
            "demo",
            "delegate",
            Policy {
                actors: ["developer".into()].into(),
                approvers: Default::default(),
                require_approval: false,
            },
        )
        .unwrap();
    let lead_ref = lead.reference();
    store.publish_agent(lead).unwrap();
    store
        .bind_coordination_policy("demo", &lead_ref, &policy)
        .unwrap();
    let mut parent = Runtime::new(
        store,
        AgentLoop,
        Parent {
            calls: calls(tasks),
        },
        registry,
    );
    let id = parent
        .submit(&fixtures::actor(), lead_ref, json!("coordinate"), None)
        .unwrap();
    for _ in 0..20 {
        parent.tick(&fixtures::actor(), id).unwrap();
        if parent
            .store
            .operations(&fixtures::actor(), id)
            .unwrap()
            .iter()
            .filter(|op| matches!(op.request, OperationRequest::Tool { .. }))
            .all(|op| {
                !matches!(
                    op.status,
                    OperationStatus::Pending
                        | OperationStatus::Running
                        | OperationStatus::Unknown
                        | OperationStatus::WaitingApproval
                )
            })
            && parent
                .store
                .operations(&fixtures::actor(), id)
                .unwrap()
                .len()
                > 1
        {
            break;
        }
    }
    (parent, id, worker_ref)
}
fn drive(mut runtime: Runtime<AgentLoop, Child, ToolRegistry>, id: Uuid) -> RunView {
    for _ in 0..40 {
        let view = runtime.tick(&fixtures::actor(), id).unwrap();
        if view.status.terminal() {
            return view;
        }
    }
    panic!("child did not finish")
}
fn keyed(store: &Store, parent: Uuid, key: &str) -> Uuid {
    store
        .children(&fixtures::actor(), parent)
        .unwrap()
        .into_iter()
        .find(|child| {
            store
                .team_task(&fixtures::actor(), child.id)
                .unwrap()
                .unwrap()
                .task_key
                == key
        })
        .unwrap()
        .id
}
#[test]
fn independent_children_overlap_dependents_wait_and_receive_evidence() {
    let store = Store::default();
    let (_, parent, _) = prepare(
        store.clone(),
        vec![
            json!({"task":"collect A","task_key":"a"}),
            json!({"task":"analyze A","task_key":"b","depends_on":["a"]}),
            json!({"task":"collect C","task_key":"c"}),
        ],
        CoordinationPolicy::default(),
    );
    let a = keyed(&store, parent, "a");
    let b = keyed(&store, parent, "b");
    let c = keyed(&store, parent, "c");
    let instructions = Arc::new(Mutex::new(vec![]));
    let mut dependent = child(store.clone(), None, instructions.clone(), false);
    for _ in 0..3 {
        assert_eq!(
            dependent.tick(&fixtures::actor(), b).unwrap().status,
            RunStatus::Queued
        );
    }
    assert!(store.operations(&fixtures::actor(), b).unwrap().is_empty());
    let barrier = Arc::new(Gate::default());
    let left = child(
        store.clone(),
        Some(barrier.clone()),
        Default::default(),
        false,
    );
    let right = child(store.clone(), Some(barrier), Default::default(), false);
    let left = std::thread::spawn(move || drive(left, a));
    let right = std::thread::spawn(move || drive(right, c));
    assert_eq!(left.join().unwrap().status, RunStatus::Completed);
    assert_eq!(right.join().unwrap().status, RunStatus::Completed);
    assert_eq!(drive(dependent, b).status, RunStatus::Completed);
    let text = instructions.lock().unwrap().join("\n");
    assert!(text.contains("source-evidence"));
    assert!(text.contains(&a.to_string()));
}
#[test]
fn failed_prerequisite_settles_dependents_without_effects_and_cancel_stops_tree() {
    let store = Store::default();
    let (runtime, parent, _) = prepare(
        store.clone(),
        vec![
            json!({"task":"fail","task_key":"a"}),
            json!({"task":"blocked","task_key":"b","depends_on":["a"]}),
        ],
        CoordinationPolicy::default(),
    );
    let a = keyed(&store, parent, "a");
    let b = keyed(&store, parent, "b");
    assert_eq!(
        drive(child(store.clone(), None, Default::default(), true), a).status,
        RunStatus::Failed
    );
    let mut dependent = child(store.clone(), None, Default::default(), false);
    let failed = dependent.tick(&fixtures::actor(), b).unwrap();
    assert_eq!(failed.status, RunStatus::Failed);
    assert!(failed.reason.unwrap().contains(&a.to_string()));
    assert!(store.operations(&fixtures::actor(), b).unwrap().is_empty());
    runtime.cancel(&fixtures::actor(), parent).unwrap();
    let store = Store::default();
    let (runtime, parent, _) = prepare(
        store.clone(),
        vec![
            json!({"task":"a","task_key":"a"}),
            json!({"task":"b","task_key":"b","depends_on":["a"]}),
        ],
        CoordinationPolicy::default(),
    );
    runtime.cancel(&fixtures::actor(), parent).unwrap();
    for child in store.children(&fixtures::actor(), parent).unwrap() {
        assert_eq!(child.status, RunStatus::Cancelled);
    }
}
#[test]
fn invalid_dependencies_repetition_and_child_budgets_are_enforced() {
    let store = Store::default();
    let (_, parent, _) = prepare(
        store.clone(),
        vec![
            json!({"task":"same","task_key":"a","max_model_calls":1}),
            json!({"task":"same","task_key":"b"}),
            json!({"task":"missing","task_key":"c","depends_on":["unknown"]}),
            json!({"task":"self","task_key":"d","depends_on":["d"]}),
        ],
        CoordinationPolicy {
            max_repeated_task: 1,
            ..Default::default()
        },
    );
    let children = store.children(&fixtures::actor(), parent).unwrap();
    assert_eq!(children.len(), 1);
    let errors: Vec<_> = store
        .operations(&fixtures::actor(), parent)
        .unwrap()
        .into_iter()
        .filter(|op| matches!(op.request, OperationRequest::Tool { .. }))
        .collect();
    let encoded = serde_json::to_string(&errors).unwrap();
    assert!(encoded.contains("no new task progress"));
    assert!(encoded.contains("forward references and cycles"));
    // This child emits a candidate that cannot pass the required output schema;
    // its per-delegation one-call limit stops verification repair.
    let mut runtime = child(store.clone(), None, Default::default(), false);
    let id = children[0].id;
    assert_eq!(
        runtime
            .tick(&fixtures::actor(), id)
            .unwrap()
            .usage
            .model_calls,
        1
    );
    let operations = store.operations(&fixtures::actor(), id).unwrap();
    assert_eq!(operations.len(), 1);
    let stopped = drive(runtime, id);
    assert_eq!(stopped.status, RunStatus::Failed);
    assert_eq!(stopped.usage.model_calls, 1);
    assert!(stopped.reason.unwrap().contains("budget"));
}
#[test]
#[ignore = "requires HUDSON_TEST_DATABASE in local PostgreSQL"]
fn dependency_graph_survives_postgres_reconnect() {
    let db = std::env::var("HUDSON_TEST_DATABASE").unwrap();
    let ns = format!("team-{}", Uuid::new_v4());
    let connect = || Store::postgres_local("/tmp", &db, &ns).unwrap();
    let store = connect();
    let (_, parent, _) = prepare(
        store.clone(),
        vec![
            json!({"task":"a","task_key":"a"}),
            json!({"task":"b","task_key":"b","depends_on":["a"]}),
        ],
        CoordinationPolicy::default(),
    );
    let a = keyed(&store, parent, "a");
    let b = keyed(&store, parent, "b");
    drop(store);
    let store = connect();
    assert_eq!(
        store
            .team_task(&fixtures::actor(), b)
            .unwrap()
            .unwrap()
            .dependencies,
        vec![a]
    );
    let mut blocked = child(store.clone(), None, Default::default(), false);
    assert_eq!(
        blocked.tick(&fixtures::actor(), b).unwrap().status,
        RunStatus::Queued
    );
    assert_eq!(
        drive(child(store.clone(), None, Default::default(), false), a).status,
        RunStatus::Completed
    );
    assert_eq!(drive(blocked, b).status, RunStatus::Completed);
}

#[test]
fn team_limit_and_idempotent_submission_prevent_extra_children() {
    let store = Store::default();
    let (_, parent, worker_ref) = prepare(
        store.clone(),
        vec![
            json!({"task":"a","task_key":"a"}),
            json!({"task":"b","task_key":"b"}),
            json!({"task":"c","task_key":"c"}),
        ],
        CoordinationPolicy {
            max_children: 2,
            max_repeated_task: 1,
            ..Default::default()
        },
    );
    assert_eq!(store.children(&fixtures::actor(), parent).unwrap().len(), 2);
    assert!(
        serde_json::to_string(&store.operations(&fixtures::actor(), parent).unwrap())
            .unwrap()
            .contains("delegation budget exhausted")
    );
    let id = keyed(&store, parent, "a");
    let task = store.team_task(&fixtures::actor(), id).unwrap().unwrap();
    let runtime = child(store.clone(), None, Default::default(), false);
    let options = hudson_core::coordination::DelegationOptions {
        task_key: Some("a".into()),
        ..Default::default()
    };
    assert_eq!(
        runtime
            .submit_delegated(
                &fixtures::actor(),
                worker_ref.clone(),
                json!("a"),
                task.parent_operation,
                options.clone()
            )
            .unwrap(),
        id
    );
    assert!(runtime
        .submit_delegated(
            &fixtures::actor(),
            worker_ref,
            json!("changed"),
            task.parent_operation,
            options
        )
        .is_err());
    assert_eq!(store.children(&fixtures::actor(), parent).unwrap().len(), 2);
}

#[test]
fn cancelled_prerequisite_fails_queued_dependent() {
    let store = Store::default();
    let (_, parent, _) = prepare(
        store.clone(),
        vec![
            json!({"task":"a","task_key":"a"}),
            json!({"task":"b","task_key":"b","depends_on":["a"]}),
        ],
        CoordinationPolicy::default(),
    );
    let a = keyed(&store, parent, "a");
    let b = keyed(&store, parent, "b");
    let mut runtime = child(store.clone(), None, Default::default(), false);
    runtime.cancel(&fixtures::actor(), a).unwrap();
    assert_eq!(
        runtime.tick(&fixtures::actor(), b).unwrap().status,
        RunStatus::Failed
    );
    assert!(store.operations(&fixtures::actor(), b).unwrap().is_empty());
}
