use hudson_core::{configured::Configuration, models::Actor, storage::Store};
use std::sync::{Arc, Barrier};

#[test]
#[ignore = "requires HUDSON_TEST_DATABASE in local PostgreSQL"]
fn concurrent_publication_reconstructs_after_connections_close() {
    let database = std::env::var("HUDSON_TEST_DATABASE").expect("set isolated test database");
    let namespace = format!("publication-{}", uuid::Uuid::new_v4());
    let path = std::env::temp_dir().join(format!("{namespace}.json"));
    std::fs::write(
        &path,
        r#"{"name":"published","instructions":"help","provider":"ollama","model":"fixture"}"#,
    )
    .unwrap();
    let actor = Actor {
        workspace_id: "company".into(),
        id: "owner".into(),
    };
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (database, namespace, path, actor, barrier) = (
                database.clone(),
                namespace.clone(),
                path.clone(),
                actor.clone(),
                barrier.clone(),
            );
            std::thread::spawn(move || {
                let store = Store::postgres_local("/tmp", &database, &namespace).unwrap();
                let config = Configuration::load(&path).unwrap();
                barrier.wait();
                store
                    .publish_configuration(&actor, &config, "same-request")
                    .unwrap()
            })
        })
        .collect();
    let receipts: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(receipts[0], receipts[1]);
    std::fs::remove_file(path).unwrap();
    let store = Store::postgres_local("/tmp", &database, &namespace).unwrap();
    let restored = store
        .published_configuration(&actor, &receipts[0].agent_ref)
        .unwrap();
    let tree = restored.build_temporal_tree(store.clone(), &actor).unwrap();
    assert_eq!(tree.reference, receipts[0].agent_ref);
    let run = tree
        .runtime
        .submit(
            &actor,
            tree.reference.clone(),
            serde_json::json!("task"),
            None,
        )
        .unwrap();
    drop(tree);
    drop(store);
    let store = Store::postgres_local("/tmp", &database, &namespace).unwrap();
    let restored = store
        .published_configuration(&actor, &receipts[0].agent_ref)
        .unwrap();
    assert_eq!(
        store
            .publish_configuration(&actor, &restored, "retry-after-run")
            .unwrap(),
        receipts[0]
    );
    let tree = restored.build_temporal_tree(store.clone(), &actor).unwrap();
    store
        .validate_resume(&actor, run, &tree.reference, &None)
        .unwrap();
}
