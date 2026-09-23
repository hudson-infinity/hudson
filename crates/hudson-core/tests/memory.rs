use hudson_core::{memory::*, models::Actor, storage::Store};
fn actor() -> Actor {
    Actor {
        id: "alice".into(),
        workspace_id: "company".into(),
    }
}
fn scope() -> MemoryScope {
    MemoryScope {
        name: "sales".into(),
    }
}
fn fact(text: &str) -> RetainMemory {
    RetainMemory {
        text: text.into(),
        kind: MemoryKind::UserFact,
        sources: vec![MemorySource {
            reference: "user:requirements".into(),
            run_id: None,
            operation_id: None,
        }],
        supersedes: None,
    }
}
#[test]
fn scoped_ranked_recall_correction_and_delete() {
    let store = Store::default();
    let a = actor();
    let s = scope();
    let old = store
        .retain_memory(
            &a,
            &s,
            "one",
            fact("Austin warehouse maximum purchase budget is three million dollars"),
        )
        .unwrap();
    store
        .retain_memory(&a, &s, "two", fact("Boston office lease requires parking"))
        .unwrap();
    store
        .retain_memory(
            &a,
            &s,
            "three",
            fact("Austin residential apartment market report"),
        )
        .unwrap();
    let hits = store
        .recall_memory(&a, &s, "warehouse budgets Austin", 2)
        .unwrap();
    assert_eq!(hits[0].id, old.id);
    let mut replacement = fact("Austin warehouse maximum purchase budget is four million dollars");
    replacement.supersedes = Some(old.id);
    let new = store
        .retain_memory(&a, &s, "four", replacement.clone())
        .unwrap();
    assert_eq!(
        store.retain_memory(&a, &s, "four", replacement).unwrap(),
        new
    );
    assert!(store.retain_memory(&a, &s, "four", fact("other")).is_err());
    let hits = store.recall_memory(&a, &s, "warehouse budget", 20).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, new.id);
    for stranger in [
        Actor {
            id: "bob".into(),
            ..a.clone()
        },
        Actor {
            workspace_id: "other".into(),
            ..a.clone()
        },
    ] {
        assert!(store
            .recall_memory(&stranger, &s, "Austin warehouse", 20)
            .unwrap()
            .is_empty());
        assert!(store.delete_memory(&stranger, &s, new.id).is_err());
        let mut correction = fact("stolen");
        correction.supersedes = Some(new.id);
        assert!(store.retain_memory(&stranger, &s, "x", correction).is_err());
    }
    assert!(store
        .recall_memory(
            &a,
            &MemoryScope {
                name: "private".into()
            },
            "Austin",
            20
        )
        .unwrap()
        .is_empty());
    store.delete_memory(&a, &s, new.id).unwrap();
    assert!(store
        .recall_memory(&a, &s, "warehouse", 20)
        .unwrap()
        .is_empty());
}
#[test]
fn provenance_bounds_and_success_labels() {
    let store = Store::default();
    let mut input = fact("claim");
    input.kind = MemoryKind::SuccessfulOutcome;
    assert!(store
        .retain_memory(&actor(), &scope(), "one", input)
        .is_err());
    let mut input = fact("claim");
    input.sources[0].run_id = Some(uuid::Uuid::new_v4());
    assert!(store
        .retain_memory(&actor(), &scope(), "one", input)
        .is_err());
    assert!(store
        .recall_memory(&actor(), &scope(), "claim", 21)
        .is_err());
}
#[test]
#[ignore = "requires HUDSON_TEST_DATABASE in local PostgreSQL"]
fn memory_survives_postgres_reopen() {
    let db = std::env::var("HUDSON_TEST_DATABASE").unwrap();
    let ns = format!("memory-{}", uuid::Uuid::new_v4());
    let store = Store::postgres_local("/tmp", &db, &ns).unwrap();
    let id = store
        .retain_memory(
            &actor(),
            &scope(),
            "one",
            fact("Warehouse investment in Austin requires a loading dock"),
        )
        .unwrap()
        .id;
    drop(store);
    let store = Store::postgres_local("/tmp", &db, &ns).unwrap();
    assert_eq!(
        store
            .recall_memory(&actor(), &scope(), "Austin warehouse docks", 3)
            .unwrap()[0]
            .id,
        id
    );
}

#[test]
fn registered_tools_keep_write_policy_and_idempotency() {
    use hudson_core::{
        adapters::tools::{Invocation, ToolExecutor, ToolRegistry},
        models::Effect,
    };
    let store = Store::default();
    let mut registry = ToolRegistry::new();
    let tools = register_tools(
        &mut registry,
        store.clone(),
        actor(),
        scope(),
        "read-policy",
        "write-policy",
    )
    .unwrap();
    assert_eq!(tools[0].effect, Effect::Read);
    assert_eq!(tools[1].effect, Effect::Write);
    assert_eq!(tools[1].policy_ref, "write-policy");
    assert_eq!(tools[2].effect, Effect::Write);
    let op = uuid::Uuid::new_v4();
    let input = serde_json::to_value(fact("Austin warehouse budget")).unwrap();
    let invoke = |registry: &mut ToolRegistry| {
        registry
            .execute(Invocation {
                operation_id: op,
                tool: &tools[1],
                arguments: &input,
            })
            .unwrap()
    };
    assert_eq!(invoke(&mut registry), invoke(&mut registry));
    assert_eq!(
        store
            .recall_memory(&actor(), &scope(), "warehouse", 20)
            .unwrap()
            .len(),
        1
    );
}

#[test]
#[cfg(feature = "fixtures")]
#[ignore = "requires HUDSON_TEST_DATABASE in local PostgreSQL"]
fn completed_run_provenance_survives_reopen_for_next_run() {
    use hudson_core::{
        fixtures::{self, FixtureBackend, FixtureTools, OrderModel},
        models::RunStatus,
        runtime::Runtime,
    };
    let db = std::env::var("HUDSON_TEST_DATABASE").unwrap();
    let ns = format!("memory-runs-{}", uuid::Uuid::new_v4());
    let store = Store::postgres_local("/tmp", &db, &ns).unwrap();
    let (agent, tools) = fixtures::definitions();
    for tool in tools {
        store.publish_tool(tool).unwrap();
    }
    store.publish_agent(agent).unwrap();
    store
        .set_policy(
            "demo",
            "lookup_order",
            hudson_core::security::Policy {
                actors: ["developer".into()].into(),
                approvers: ["developer".into()].into(),
                require_approval: false,
            },
        )
        .unwrap();
    let mut runtime = Runtime::new(store, FixtureBackend, OrderModel, FixtureTools::default());
    let a = fixtures::actor();
    let input = serde_json::json!({"order_id":"123","action":"lookup"});
    let id = runtime
        .submit(&a, fixtures::agent_ref(), input.clone(), None)
        .unwrap();
    let mut content = fact("Order lookup completed for customer 123");
    content.kind = MemoryKind::SuccessfulOutcome;
    content.sources[0].run_id = Some(id);
    assert!(runtime
        .store
        .retain_memory(&a, &scope(), "result", content.clone())
        .is_err());
    let view = fixtures::drive(&mut runtime, &a, id).unwrap();
    assert_eq!(view.status, RunStatus::Completed);
    runtime
        .store
        .retain_memory(&a, &scope(), "result", content)
        .unwrap();
    drop(runtime);
    let store = Store::postgres_local("/tmp", &db, &ns).unwrap();
    let runtime = Runtime::new(store, FixtureBackend, OrderModel, FixtureTools::default());
    let next = runtime
        .submit(&a, fixtures::agent_ref(), input, None)
        .unwrap();
    assert_ne!(next, id);
    let recalled = runtime
        .store
        .recall_memory(&a, &scope(), "customer order lookup", 5)
        .unwrap();
    assert_eq!(recalled.len(), 1);
    assert_eq!(recalled[0].content.sources[0].run_id, Some(id));
}

#[test]
fn recall_is_bounded_by_serialized_bytes() {
    let store = Store::default();
    for n in 0..12 {
        store
            .retain_memory(
                &actor(),
                &scope(),
                &format!("large-{n}"),
                fact(&format!("warehouse {}", "detail ".repeat(1000))),
            )
            .unwrap();
    }
    let hits = store
        .recall_memory(&actor(), &scope(), "warehouse", 20)
        .unwrap();
    assert!(!hits.is_empty());
    assert!(hits.len() < 12);
    assert!(serde_json::to_vec(&hits).unwrap().len() <= 32 * 1024);
}
