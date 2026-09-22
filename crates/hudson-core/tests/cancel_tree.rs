#![cfg(feature = "fixtures")]
use hudson_core::{fixtures, models::*};
use serde_json::json;
#[test]
fn cancelling_parent_stops_attached_children_and_prevents_late_attachment() {
    let mut runtime = fixtures::runtime().unwrap();
    let actor = fixtures::actor();
    let input = json!({"order_id":"123","action":"lookup"});
    let parent = runtime
        .submit(&actor, fixtures::agent_ref(), input.clone(), None)
        .unwrap();
    runtime.tick(&actor, parent).unwrap();
    let op = runtime.store.operations(&actor, parent).unwrap()[0].meta.id;
    let child = runtime
        .submit(&actor, fixtures::agent_ref(), input.clone(), None)
        .unwrap();
    assert!(runtime.store.link_child(&actor, child, op).unwrap());
    assert_eq!(
        runtime
            .store
            .inspect(&actor, child)
            .unwrap()
            .parent_operation,
        Some(op)
    );
    runtime
        .store
        .validate_child_access(&actor, child, op, &fixtures::agent_ref())
        .unwrap();
    let other = runtime
        .submit(&actor, fixtures::agent_ref(), input.clone(), None)
        .unwrap();
    runtime.tick(&actor, other).unwrap();
    let other_op = runtime.store.operations(&actor, other).unwrap()[0].meta.id;
    assert!(runtime
        .store
        .validate_child_access(&actor, child, other_op, &fixtures::agent_ref())
        .is_err());
    runtime.cancel(&actor, parent).unwrap();
    assert_eq!(
        runtime.store.inspect(&actor, parent).unwrap().status,
        RunStatus::Cancelled
    );
    assert_eq!(
        runtime.store.inspect(&actor, child).unwrap().status,
        RunStatus::Cancelled
    );
    for run_id in [parent, child] {
        let events = runtime.store.events(&actor, run_id, 0).unwrap();
        let cancelled = events
            .iter()
            .filter(|event| event.event_type == EventType::CancellationRequested)
            .collect::<Vec<_>>();
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].payload["root_run"], json!(parent));
        assert_eq!(cancelled[0].actor_id.as_deref(), Some(actor.id.as_str()));
    }
    let late = runtime
        .submit(&actor, fixtures::agent_ref(), input, None)
        .unwrap();
    assert!(runtime.store.link_child(&actor, late, op).is_err());
    assert!(runtime.store.operations(&actor, child).unwrap().is_empty());
}
