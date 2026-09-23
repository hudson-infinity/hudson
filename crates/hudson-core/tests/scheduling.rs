#![cfg(feature = "fixtures")]
use hudson_core::{fixtures, models::Actor, scheduling::ScheduleTarget};
use serde_json::json;

#[test]
fn scheduling_intent_is_atomic_scoped_and_idempotent() {
    let runtime = fixtures::runtime().unwrap();
    let actor = fixtures::actor();
    let target = ScheduleTarget {
        scheduler: "temporal:local".into(),
        task_queue: "agents".into(),
    };
    let input = json!({"order_id":"123","action":"lookup"});
    let submit = |destination| {
        runtime.submit_scheduled(
            &actor,
            fixtures::agent_ref(),
            input.clone(),
            Some("key".into()),
            None,
            destination,
        )
    };
    let id = submit(Some(target.clone())).unwrap();
    assert_eq!(submit(Some(target.clone())).unwrap(), id);
    assert!(submit(None).is_err());
    let other = ScheduleTarget {
        task_queue: "other".into(),
        ..target.clone()
    };
    assert!(submit(Some(other.clone())).is_err());
    assert_eq!(
        runtime
            .store
            .pending_schedules(&actor, &fixtures::agent_ref(), &target, 10)
            .unwrap(),
        vec![id]
    );
    let stranger = Actor {
        id: "stranger".into(),
        ..actor.clone()
    };
    assert!(runtime
        .store
        .pending_schedules(&stranger, &fixtures::agent_ref(), &target, 10)
        .unwrap()
        .is_empty());
    assert!(runtime
        .store
        .acknowledge_schedule(&stranger, id, &target)
        .is_err());
    assert!(runtime
        .store
        .acknowledge_schedule(&actor, id, &other)
        .is_err());
    runtime
        .store
        .acknowledge_schedule(&actor, id, &target)
        .unwrap();
    runtime
        .store
        .acknowledge_schedule(&actor, id, &target)
        .unwrap();
    assert!(runtime
        .store
        .pending_schedules(&actor, &fixtures::agent_ref(), &target, 10)
        .unwrap()
        .is_empty());
    assert_eq!(submit(Some(target)).unwrap(), id);
}

#[test]
fn invalid_submissions_and_ordinary_runs_never_enter_scheduler_queue() {
    let runtime = fixtures::runtime().unwrap();
    let actor = fixtures::actor();
    let target = ScheduleTarget {
        scheduler: "temporal:local".into(),
        task_queue: "agents".into(),
    };
    assert!(runtime
        .submit_scheduled(
            &actor,
            fixtures::agent_ref(),
            json!({}),
            None,
            None,
            Some(target.clone())
        )
        .is_err());
    runtime
        .submit(
            &actor,
            fixtures::agent_ref(),
            json!({"order_id":"123","action":"lookup"}),
            None,
        )
        .unwrap();
    assert!(runtime
        .store
        .pending_schedules(&actor, &fixtures::agent_ref(), &target, 10)
        .unwrap()
        .is_empty());
}
