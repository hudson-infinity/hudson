use hudson_core::{
    models::{now, Actor},
    storage::Store,
};

#[test]
#[ignore = "requires the dedicated local PostgreSQL test database"]
fn token_issuance_and_revocation_survive_independent_connections() {
    let database = std::env::var("HUDSON_TEST_DATABASE")
        .unwrap_or_else(|_| "hudson_harness_test_20260921".into());
    let namespace = format!("credentials-{}", uuid::Uuid::new_v4());
    let issuer = Store::postgres_local("/tmp", &database, &namespace).unwrap();
    let actor = Actor {
        workspace_id: "customer-one".into(),
        id: "backend".into(),
    };
    let token = issuer
        .issue_api_token(actor.clone(), "backend".into(), now() + 60_000)
        .unwrap();
    let verifier = Store::postgres_local("/tmp", &database, &namespace).unwrap();
    assert_eq!(
        verifier
            .authenticate_api_token(token.bearer())
            .unwrap()
            .workspace_id,
        actor.workspace_id
    );
    drop(issuer);
    let operator = Store::postgres_local("/tmp", &database, &namespace).unwrap();
    operator
        .revoke_api_token(&actor, token.metadata.id)
        .unwrap();
    assert!(
        verifier.authenticate_api_token(token.bearer()).is_err(),
        "an existing verifier must observe revocation"
    );
    drop(operator);
    drop(verifier);
    let restarted = Store::postgres_local("/tmp", &database, &namespace).unwrap();
    assert!(restarted.authenticate_api_token(token.bearer()).is_err());
}
