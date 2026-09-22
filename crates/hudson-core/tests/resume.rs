#![cfg(feature = "fixtures")]
use hudson_core::{fixtures, models::*};
use serde_json::json;
#[test]
fn resume_requires_pinned_agent_goal_and_model_transport() {
    let runtime = fixtures::runtime().unwrap();
    let actor = fixtures::actor();
    let agent = fixtures::agent_ref();
    runtime
        .store
        .bind_model_transport(&actor.workspace_id, &agent, "original")
        .unwrap();
    runtime
        .store
        .bind_model_transport(&actor.workspace_id, &agent, "original")
        .unwrap();
    assert!(runtime
        .store
        .bind_model_transport(&actor.workspace_id, &agent, "other-endpoint")
        .is_err());
    let id = runtime
        .submit(
            &actor,
            agent.clone(),
            json!({"order_id":"123","action":"lookup"}),
            None,
        )
        .unwrap();
    runtime
        .store
        .validate_resume(&actor, id, &agent, &None)
        .unwrap();
    assert!(runtime
        .store
        .validate_resume(
            &actor,
            id,
            &VersionRef {
                id: agent.id.clone(),
                version: 2
            },
            &None
        )
        .is_err());
    assert!(runtime
        .store
        .validate_resume(
            &actor,
            id,
            &agent,
            &Some(Goal {
                objective: "different".into(),
                success_schema: json!(true)
            })
        )
        .is_err());
    assert!(runtime
        .store
        .validate_resume(
            &Actor {
                workspace_id: actor.workspace_id,
                id: "other".into()
            },
            id,
            &agent,
            &None
        )
        .is_err());
}
