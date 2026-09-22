use hudson_harness::*;
use serde_json::json;

struct ExampleBackend;
impl Backend for ExampleBackend {
    fn name(&self) -> &str {
        "test"
    }
    fn version(&self) -> u32 {
        1
    }
    fn advance(
        &self,
        _: &Config,
        state: &Checkpoint,
        _: Input,
    ) -> Result<Transition, HarnessError> {
        Ok(Transition {
            checkpoint: Checkpoint {
                step: state.step + 1,
                ..state.clone()
            },
            action: Action::WaitForInput {
                prompt: "Which order?".into(),
            },
        })
    }
}
fn config() -> Config {
    Config {
        allow_user_input: false,
        instructions: "help".into(),
        model: "test".into(),
        tools: vec![],
        max_context_bytes: 4096,
    }
}
#[test]
fn rejects_foreign_and_future_checkpoints() {
    let engine = Engine::new(ExampleBackend);
    for state in [
        Checkpoint {
            schema_version: 2,
            ..engine.initial_state()
        },
        Checkpoint {
            backend_version: 2,
            ..engine.initial_state()
        },
        Checkpoint {
            backend: "other".into(),
            ..engine.initial_state()
        },
    ] {
        assert!(matches!(
            engine.advance(
                &config(),
                &state,
                Input::Start {
                    value: json!("help")
                }
            ),
            Err(HarnessError::IncompatibleCheckpoint)
        ));
    }
}
#[test]
fn rejects_a_backend_that_does_not_advance_state() {
    struct Broken;
    impl Backend for Broken {
        fn name(&self) -> &str {
            "broken"
        }
        fn version(&self) -> u32 {
            1
        }
        fn advance(
            &self,
            _: &Config,
            state: &Checkpoint,
            _: Input,
        ) -> Result<Transition, HarnessError> {
            Ok(Transition {
                checkpoint: state.clone(),
                action: Action::Fail { reason: "x".into() },
            })
        }
    }
    let engine = Engine::new(Broken);
    assert!(engine
        .advance(
            &config(),
            &engine.initial_state(),
            Input::Start { value: json!(null) }
        )
        .is_err());
}
#[test]
fn oversized_context_fails_instead_of_truncating_input() {
    let mut config = config();
    config.max_context_bytes = 10;
    assert!(matches!(
        context::build(
            &config,
            vec![Message {
                role: Role::User,
                content: vec![Content::Text {
                    text: "required information".into()
                }]
            }]
        ),
        Err(HarnessError::ContextLimit)
    ));
}
#[test]
fn checkpoint_and_protocol_roundtrip() {
    let engine = Engine::new(ExampleBackend);
    let transition = engine
        .advance(
            &config(),
            &engine.initial_state(),
            Input::Start {
                value: json!({"task":"help"}),
            },
        )
        .unwrap();
    let restored: Transition =
        serde_json::from_slice(&serde_json::to_vec(&transition).unwrap()).unwrap();
    assert_eq!(transition, restored);
}
