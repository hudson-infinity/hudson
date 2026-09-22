use hudson_core::{
    adapters::{models::ModelExecutor, tools::ExecutionError},
    budgets::BudgetedModel,
    storage::Store,
};
use hudson_harness::{ModelRequest, ModelResponse};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
struct Model(Arc<AtomicUsize>);
impl ModelExecutor for Model {
    fn call(&mut self, _: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ExecutionError::Unknown("simulated timeout".into()))
    }
}
#[test]
fn concurrent_children_share_one_budget_and_reopening_does_not_reset_it() {
    let store = Store::default();
    let count = Arc::new(AtomicUsize::new(0));
    let models = (0..8)
        .map(|_| {
            BudgetedModel::new(
                Model(count.clone()),
                store.clone(),
                "workspace",
                "family",
                3,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let handles = models
        .into_iter()
        .map(|mut model| {
            std::thread::spawn(move || {
                let _ = model.call(&ModelRequest {
                    model: "test".into(),
                    instructions: String::new(),
                    messages: vec![],
                    tools: vec![],
                });
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(count.load(Ordering::SeqCst), 3);
    let reopened = BudgetedModel::new(
        Model(count.clone()),
        store.clone(),
        "workspace",
        "family",
        3,
    )
    .unwrap();
    assert_eq!(reopened.usage().unwrap().admitted, 3);
    assert!(BudgetedModel::new(Model(count), store, "workspace", "family", 4).is_err());
}

#[test]
#[ignore = "requires HUDSON_TEST_DATABASE"]
fn postgres_reconnect_preserves_shared_budget() {
    let database = std::env::var("HUDSON_TEST_DATABASE").unwrap();
    let namespace = format!("budget-{}", uuid::Uuid::new_v4());
    let count = Arc::new(AtomicUsize::new(0));
    let request = ModelRequest {
        model: "test".into(),
        instructions: String::new(),
        messages: vec![],
        tools: vec![],
    };
    {
        let store = Store::postgres_local("/tmp", &database, &namespace).unwrap();
        let mut model =
            BudgetedModel::new(Model(count.clone()), store, "workspace", "group", 1).unwrap();
        assert!(model.call(&request).is_err());
    }
    let store = Store::postgres_local("/tmp", &database, &namespace).unwrap();
    let mut model =
        BudgetedModel::new(Model(count.clone()), store, "workspace", "group", 1).unwrap();
    assert_eq!(model.usage().unwrap().admitted, 1);
    assert!(model.call(&request).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "fixtures")]
#[test]
fn saved_run_rejects_changed_or_removed_budget_but_new_batches_are_allowed() {
    use hudson_core::{
        adapters::tools::ToolRegistry,
        fixtures::{self, FixtureBackend},
        runtime::Runtime,
    };
    let store = fixtures::store().unwrap();
    let actor = fixtures::actor();
    let count = Arc::new(AtomicUsize::new(0));
    let make = |group: &str| {
        Runtime::new(
            store.clone(),
            FixtureBackend,
            Box::new(
                BudgetedModel::new(
                    Model(count.clone()),
                    store.clone(),
                    &actor.workspace_id,
                    group,
                    2,
                )
                .unwrap(),
            ) as Box<dyn ModelExecutor>,
            ToolRegistry::new(),
        )
    };
    let mut original = make("original");
    let input = serde_json::json!({"order_id":"123","action":"lookup"});
    let id = original
        .submit(
            &actor,
            fixtures::agent_ref(),
            input.clone(),
            Some("task".into()),
        )
        .unwrap();
    let mut changed = make("new-batch");
    assert!(changed.tick(&actor, id).is_err());
    assert!(changed
        .submit(
            &actor,
            fixtures::agent_ref(),
            input.clone(),
            Some("task".into())
        )
        .is_err());
    let mut removed = Runtime::new(
        store.clone(),
        FixtureBackend,
        Model(count.clone()),
        ToolRegistry::new(),
    );
    assert!(removed.tick(&actor, id).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(
        original
            .submit(
                &actor,
                fixtures::agent_ref(),
                input.clone(),
                Some("task".into())
            )
            .unwrap(),
        id
    );
    // A new run may intentionally start another batch using the same Agent version.
    let new_id = changed
        .submit(&actor, fixtures::agent_ref(), input, None)
        .unwrap();
    assert_ne!(new_id, id);
    changed.tick(&actor, new_id).unwrap();
    original.tick(&actor, id).unwrap();
    changed.tick(&actor, new_id).unwrap();
    original.tick(&actor, id).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
    // Restoring the original binding works without creating a replacement run.
    let mut reopened = make("original");
    reopened.tick(&actor, id).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[cfg(feature = "fixtures")]
#[test]
#[ignore = "requires HUDSON_TEST_DATABASE"]
fn postgres_run_budget_binding_survives_reconnect() {
    use hudson_core::{
        adapters::tools::ToolRegistry,
        fixtures::{self, FixtureBackend},
        runtime::Runtime,
    };
    let database = std::env::var("HUDSON_TEST_DATABASE").unwrap();
    let namespace = format!("run-budget-{}", uuid::Uuid::new_v4());
    let connect = || Store::postgres_local("/tmp", &database, &namespace).unwrap();
    let actor = fixtures::actor();
    let count = Arc::new(AtomicUsize::new(0));
    let id = {
        let store = connect();
        let (mut agent, _) = fixtures::definitions();
        agent.tools.clear();
        store.publish_agent(agent).unwrap();
        let model = BudgetedModel::new(
            Model(count.clone()),
            store.clone(),
            &actor.workspace_id,
            "first",
            2,
        )
        .unwrap();
        let runtime = Runtime::new(store, FixtureBackend, model, ToolRegistry::new());
        runtime
            .submit(
                &actor,
                fixtures::agent_ref(),
                serde_json::json!({"order_id":"123","action":"lookup"}),
                None,
            )
            .unwrap()
    };
    let reopen = |group| {
        let store = connect();
        let model = BudgetedModel::new(
            Model(count.clone()),
            store.clone(),
            &actor.workspace_id,
            group,
            2,
        )
        .unwrap();
        Runtime::new(store, FixtureBackend, model, ToolRegistry::new())
    };
    assert!(reopen("replacement").tick(&actor, id).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let mut original = reopen("first");
    original.tick(&actor, id).unwrap();
    original.tick(&actor, id).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
