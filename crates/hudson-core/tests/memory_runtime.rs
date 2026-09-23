#![cfg(feature = "fixtures")]
use hudson_core::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, ToolRegistry},
    },
    fixtures,
    memory::*,
    models::*,
    runtime::Runtime,
    storage::Store,
};
use hudson_harness::{agent_loop::AgentLoop, ModelRequest, ModelResponse};
use serde_json::json;
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct FinalModel {
    instructions: Arc<Mutex<Vec<String>>>,
}
impl ModelExecutor for FinalModel {
    fn call(&mut self, request: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        self.instructions
            .lock()
            .unwrap()
            .push(request.instructions.clone());
        Ok(ModelResponse::Final {
            output: json!({"memory":"Austin warehouses need loading docks","answer":"done"}),
        })
    }
}
fn configure(store: &Store, enabled: bool) {
    let (mut agent, _) = fixtures::definitions();
    agent.tools.clear();
    agent.input_schema = None;
    agent.output_schema = None;
    store.publish_agent(agent).unwrap();
    store
        .bind_memory(
            "demo",
            &fixtures::agent_ref(),
            enabled.then(|| MemoryConfig {
                scope: MemoryScope {
                    name: "project".into(),
                },
                recall_limit: 5,
                retain_pointer: Some("/memory".into()),
            }),
        )
        .unwrap();
}
fn runtime(store: Store) -> Runtime<AgentLoop, FinalModel, ToolRegistry> {
    Runtime::new(store, AgentLoop, FinalModel::default(), ToolRegistry::new())
}
fn exercise(store: Store, enabled: bool) {
    configure(&store, enabled);
    let a = fixtures::actor();
    let mut runtime = runtime(store.clone());
    let first = runtime
        .submit(
            &a,
            fixtures::agent_ref(),
            json!("Austin warehouse requirements"),
            None,
        )
        .unwrap();
    assert_eq!(
        fixtures::drive(&mut runtime, &a, first).unwrap().status,
        RunStatus::Completed
    );
    let second = runtime
        .submit(
            &a,
            fixtures::agent_ref(),
            json!("What do warehouses in Austin need?"),
            None,
        )
        .unwrap();
    assert_eq!(
        fixtures::drive(&mut runtime, &a, second).unwrap().status,
        RunStatus::Completed
    );
    let calls = runtime.store.operations(&a, second).unwrap();
    let request = calls
        .iter()
        .find_map(|op| match &op.request {
            OperationRequest::Model { request } => Some(request),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        request
            .instructions
            .contains("Austin warehouses need loading docks"),
        enabled
    );
    assert_eq!(
        request.instructions.contains("untrusted historical data"),
        enabled
    );
    let recalled = store
        .recall_memory(
            &a,
            &MemoryScope {
                name: "project".into(),
            },
            "warehouse",
            20,
        )
        .unwrap();
    assert_eq!(recalled.len(), if enabled { 2 } else { 0 });
    runtime.tick(&a, second).unwrap();
    assert_eq!(
        store
            .recall_memory(
                &a,
                &MemoryScope {
                    name: "project".into()
                },
                "warehouse",
                20
            )
            .unwrap()
            .len(),
        recalled.len()
    );
}
#[test]
fn actual_loop_retrieves_previous_selected_outcomes() {
    exercise(Store::default(), true)
}
#[test]
fn disabled_memory_does_not_retrieve_or_retain() {
    exercise(Store::default(), false)
}
#[test]
fn binding_is_immutable_and_late_enabling_is_rejected() {
    let store = Store::default();
    configure(&store, true);
    assert!(store
        .bind_memory("demo", &fixtures::agent_ref(), None)
        .is_err());
    let store = Store::default();
    let (mut agent, _) = fixtures::definitions();
    agent.tools.clear();
    store.publish_agent(agent).unwrap();
    let runtime = runtime(store.clone());
    runtime
        .submit(
            &fixtures::actor(),
            fixtures::agent_ref(),
            json!({"order_id":"1","action":"lookup"}),
            None,
        )
        .unwrap();
    assert!(store
        .bind_memory(
            "demo",
            &fixtures::agent_ref(),
            Some(MemoryConfig {
                scope: MemoryScope {
                    name: "project".into()
                },
                recall_limit: 5,
                retain_pointer: None
            })
        )
        .is_err());
}
#[test]
#[ignore = "requires HUDSON_TEST_DATABASE"]
fn persisted_first_request_and_selected_outcomes_survive_restart() {
    let db = std::env::var("HUDSON_TEST_DATABASE").unwrap();
    let ns = format!("memory-loop-{}", uuid::Uuid::new_v4());
    let connect = || Store::postgres_local("/tmp", &db, &ns).unwrap();
    let store = connect();
    configure(&store, true);
    let a = fixtures::actor();
    let mut first_runtime = runtime(store);
    let id = first_runtime
        .submit(&a, fixtures::agent_ref(), json!("Austin warehouse"), None)
        .unwrap();
    fixtures::drive(&mut first_runtime, &a, id).unwrap();
    drop(first_runtime);
    let mut second_runtime = runtime(connect());
    let next = second_runtime
        .submit(
            &a,
            fixtures::agent_ref(),
            json!("Austin warehouse requirements"),
            None,
        )
        .unwrap();
    second_runtime.tick(&a, next).unwrap();
    let before = second_runtime.store.operations(&a, next).unwrap();
    drop(second_runtime);
    let mut resumed = runtime(connect());
    assert_eq!(resumed.store.operations(&a, next).unwrap(), before);
    assert_eq!(
        fixtures::drive(&mut resumed, &a, next).unwrap().status,
        RunStatus::Completed
    );
    let ops = resumed.store.operations(&a, next).unwrap();
    assert!(ops.iter().any(|o|matches!(&o.request,OperationRequest::Model{request} if request.instructions.contains("loading docks"))));
}

#[test]
fn explicit_scope_without_selection_never_retains_final() {
    let store = Store::default();
    let (mut agent, _) = fixtures::definitions();
    agent.tools.clear();
    agent.input_schema = None;
    agent.output_schema = None;
    store.publish_agent(agent).unwrap();
    let scope = MemoryScope {
        name: "project".into(),
    };
    store
        .bind_memory(
            "demo",
            &fixtures::agent_ref(),
            Some(MemoryConfig {
                scope: scope.clone(),
                recall_limit: 5,
                retain_pointer: None,
            }),
        )
        .unwrap();
    let mut runtime = runtime(store.clone());
    let actor = fixtures::actor();
    let id = runtime
        .submit(&actor, fixtures::agent_ref(), json!("warehouse"), None)
        .unwrap();
    fixtures::drive(&mut runtime, &actor, id).unwrap();
    assert!(store
        .recall_memory(&actor, &scope, "warehouse", 20)
        .unwrap()
        .is_empty());
}
#[test]
fn recall_snapshot_is_stable_when_memory_changes_during_run() {
    let store = Store::default();
    configure(&store, true);
    let actor = fixtures::actor();
    let scope = MemoryScope {
        name: "project".into(),
    };
    let source = MemorySource {
        reference: "user".into(),
        run_id: None,
        operation_id: None,
    };
    let record = store
        .retain_memory(
            &actor,
            &scope,
            "fact",
            RetainMemory {
                text: "Austin warehouse budget three million".into(),
                kind: MemoryKind::UserFact,
                sources: vec![source.clone()],
                supersedes: None,
            },
        )
        .unwrap();
    let mut runtime = runtime(store.clone());
    let id = runtime
        .submit(
            &actor,
            fixtures::agent_ref(),
            json!("Austin warehouse budget"),
            None,
        )
        .unwrap();
    runtime.tick(&actor, id).unwrap();
    store
        .retain_memory(
            &actor,
            &scope,
            "correction",
            RetainMemory {
                text: "Austin warehouse budget four million".into(),
                kind: MemoryKind::UserFact,
                sources: vec![source],
                supersedes: Some(record.id),
            },
        )
        .unwrap();
    fixtures::drive(&mut runtime, &actor, id).unwrap();
    let requests = store.operations(&actor, id).unwrap();
    assert!(requests.iter().any(|op|matches!(&op.request,OperationRequest::Model{request} if request.instructions.contains("three million")&&!request.instructions.contains("four million"))));
    let next = runtime
        .submit(
            &actor,
            fixtures::agent_ref(),
            json!("Austin warehouse budget"),
            None,
        )
        .unwrap();
    runtime.tick(&actor, next).unwrap();
    let requests = store.operations(&actor, next).unwrap();
    assert!(requests.iter().any(|op|matches!(&op.request,OperationRequest::Model{request} if request.instructions.contains("four million")&&!request.instructions.contains("three million"))));
}

#[test]
fn shared_policy_cannot_expose_registration_actors_private_memory() {
    use hudson_core::security::Policy;
    use hudson_harness::ToolCall;
    struct Repeat(ToolRegistry);
    impl hudson_core::adapters::tools::ToolExecutor for Repeat {
        fn execute(
            &mut self,
            call: hudson_core::adapters::tools::Invocation<'_>,
        ) -> Result<serde_json::Value, ExecutionError> {
            use hudson_core::adapters::tools::Invocation;
            let first = self.0.execute(Invocation {
                operation_id: call.operation_id,
                tool: call.tool,
                arguments: call.arguments,
            });
            let repeated = self.0.execute(Invocation {
                operation_id: call.operation_id,
                tool: call.tool,
                arguments: call.arguments,
            });
            assert_eq!(first.is_ok(), repeated.is_ok());
            if let (Ok(a), Ok(b)) = (&first, &repeated) {
                assert_eq!(a, b);
            }
            let changed = json!({"query":"different","limit":5});
            assert!(self
                .0
                .execute(Invocation {
                    operation_id: call.operation_id,
                    tool: call.tool,
                    arguments: &changed
                })
                .is_err());
            first
        }
    }
    struct Recall(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl ModelExecutor for Recall {
        fn call(&mut self, _: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
            if self.0.swap(true, std::sync::atomic::Ordering::SeqCst) {
                Ok(ModelResponse::Final {
                    output: json!("finished"),
                })
            } else {
                Ok(ModelResponse::ToolCalls {
                    calls: vec![ToolCall {
                        call_id: "recall".into(),
                        name: "recall_memory".into(),
                        arguments: json!({"query":"private","limit":5}),
                        provider_metadata: serde_json::Value::Null,
                    }],
                })
            }
        }
    }
    let store = Store::default();
    let alice = fixtures::actor();
    let bob = Actor {
        id: "bob".into(),
        workspace_id: alice.workspace_id.clone(),
    };
    let scope = MemoryScope {
        name: "private".into(),
    };
    store
        .retain_memory(
            &alice,
            &scope,
            "secret",
            RetainMemory {
                text: "private confidential acquisition".into(),
                kind: MemoryKind::UserFact,
                sources: vec![MemorySource {
                    reference: "user".into(),
                    run_id: None,
                    operation_id: None,
                }],
                supersedes: None,
            },
        )
        .unwrap();
    let mut registry = ToolRegistry::new();
    let tools = register_tools(
        &mut registry,
        store.clone(),
        alice.clone(),
        scope,
        "shared",
        "shared",
    )
    .unwrap();
    let (mut agent, _) = fixtures::definitions();
    agent.tools = tools
        .iter()
        .map(|t| AgentTool {
            tool_ref: t.reference(),
            alias: t.name.clone(),
        })
        .collect();
    agent.input_schema = None;
    agent.output_schema = None;
    for tool in tools {
        store.publish_tool(tool).unwrap();
    }
    store.publish_agent(agent).unwrap();
    store
        .set_policy(
            &alice.workspace_id,
            "shared",
            Policy {
                actors: [alice.id.clone(), bob.id.clone()].into(),
                approvers: Default::default(),
                require_approval: false,
            },
        )
        .unwrap();
    let phase = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut runtime = Runtime::new(
        store.clone(),
        AgentLoop,
        Recall(phase.clone()),
        Repeat(registry),
    );
    let own = runtime
        .submit(&alice, fixtures::agent_ref(), json!("recall"), None)
        .unwrap();
    fixtures::drive(&mut runtime, &alice, own).unwrap();
    assert!(
        serde_json::to_string(&store.operations(&alice, own).unwrap())
            .unwrap()
            .contains("confidential acquisition")
    );
    phase.store(false, std::sync::atomic::Ordering::SeqCst);
    let id = runtime
        .submit(&bob, fixtures::agent_ref(), json!("recall"), None)
        .unwrap();
    fixtures::drive(&mut runtime, &bob, id).unwrap();
    let ops = store.operations(&bob, id).unwrap();
    let op = ops
        .iter()
        .find(|o| matches!(o.request, OperationRequest::Tool { .. }))
        .unwrap();
    assert_eq!(op.status, OperationStatus::Failed);
    assert!(!serde_json::to_string(&ops)
        .unwrap()
        .contains("confidential acquisition"));
}
