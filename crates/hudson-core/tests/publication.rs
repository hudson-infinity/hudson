#![cfg(feature = "fixtures")]
use hudson_core::{fixtures, storage::Store};
use std::sync::{Arc, Barrier};

fn concurrent_startup(stores: Vec<Store>) {
    let (agent, tools) = fixtures::definitions();
    let barrier = Arc::new(Barrier::new(stores.len()));
    let handles: Vec<_> = stores
        .into_iter()
        .map(|store| {
            let agent = agent.clone();
            let tools = tools.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for tool in tools {
                    store.ensure_tool(tool).unwrap();
                }
                store.ensure_agent(agent.clone()).unwrap();
                // Idempotent startup must not make administrative publication mutable.
                assert!(store.publish_agent(agent.clone()).is_err());
                let mut changed = agent;
                changed.instructions.push_str(" changed");
                assert!(store.ensure_agent(changed).is_err());
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn concurrent_identical_definitions_succeed_but_conflicting_content_cannot_replace_them() {
    for _ in 0..8 {
        let store = Store::default();
        concurrent_startup(vec![store.clone(); 8]);
        let (_, tools) = fixtures::definitions();
        for mut tool in tools {
            assert!(store.publish_tool(tool.clone()).is_err());
            tool.description.push_str(" changed");
            assert!(store.ensure_tool(tool).is_err());
        }
    }
}

#[test]
#[ignore = "requires HUDSON_TEST_DATABASE in local PostgreSQL"]
fn independent_postgres_connections_publish_identical_definitions_atomically() {
    let database = std::env::var("HUDSON_TEST_DATABASE").expect("dedicated test database required");
    let namespace = format!("publication-{}", uuid::Uuid::new_v4());
    let stores = (0..4)
        .map(|_| Store::postgres_local("/tmp", &database, &namespace).unwrap())
        .collect();
    concurrent_startup(stores);
}
