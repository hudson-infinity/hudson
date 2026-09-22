#![cfg(feature = "fixtures")]
use hudson_core::{
    fixtures::{self, FixtureBackend, FixtureTools, OrderModel},
    models::*,
    runtime::Runtime,
    storage::Store,
};

#[test]
#[ignore = "requires HUDSON_TEST_DATABASE in local PostgreSQL"]
fn reconnect_preserves_checkpoints_and_submission_identity() {
    let database = std::env::var("HUDSON_TEST_DATABASE").expect("set isolated test database");
    let namespace = format!("test-{}", uuid::Uuid::new_v4());
    let connect = || Store::postgres_local("/tmp", &database, &namespace).unwrap();
    let store = connect();
    let (mut agent, _) = fixtures::definitions();
    agent.tools.clear();
    agent.output_schema = None;
    let reference = agent.reference();
    store.publish_agent(agent).unwrap();
    let actor = fixtures::actor();
    let input = serde_json::json!({"order_id":"123", "action":"lookup"});
    let mut runtime = Runtime::new(store, FixtureBackend, OrderModel, FixtureTools::default());
    let id = runtime
        .submit(
            &actor,
            reference.clone(),
            input.clone(),
            Some("same".into()),
        )
        .unwrap();
    runtime.tick(&actor, id).unwrap();
    let before = runtime.store.events(&actor, id, 0).unwrap();
    assert!(!before.is_empty());
    drop(runtime);
    let runtime = Runtime::new(
        connect(),
        FixtureBackend,
        OrderModel,
        FixtureTools::default(),
    );
    assert_eq!(
        runtime
            .submit(&actor, reference, input, Some("same".into()))
            .unwrap(),
        id
    );
    assert_eq!(
        runtime.store.events(&actor, id, 0).unwrap().len(),
        before.len()
    );
    assert!(!runtime.store.operations(&actor, id).unwrap().is_empty());
    assert_eq!(
        runtime.store.inspect(&actor, id).unwrap().status,
        RunStatus::Running
    );
}
