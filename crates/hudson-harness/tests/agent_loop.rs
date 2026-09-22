use hudson_harness::*;
use serde_json::json;

fn config() -> Config {
    Config {
        allow_user_input: false,
        instructions: "solve the task".into(),
        model: "configured-model".into(),
        tools: vec![],
        max_context_bytes: 10000,
    }
}
fn advance(state: &Checkpoint, input: Input) -> Transition {
    // Construct a fresh engine and deserialize at every boundary, like process recovery.
    let restored = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    Engine::new(AgentLoop)
        .advance(&config(), &restored, input)
        .unwrap()
}
#[test]
fn resumes_tool_cycle_and_repairs_failed_verification() {
    let started = advance(
        &Engine::new(AgentLoop).initial_state(),
        Input::Start {
            value: json!("analyze"),
        },
    );
    let calls = vec![ToolCall {
        provider_metadata: serde_json::Value::Null,
        call_id: "a".into(),
        name: "query".into(),
        arguments: json!({}),
    }];
    let tools = advance(
        &started.checkpoint,
        Input::Model {
            response: ModelResponse::ToolCalls { calls },
        },
    );
    let model = advance(
        &tools.checkpoint,
        Input::Tools {
            results: vec![ToolResult {
                call_id: "a".into(),
                outcome: ToolOutcome::Success { value: json!(42) },
            }],
        },
    );
    let Action::CallModel { request } = model.action else {
        panic!("expected model")
    };
    assert_eq!(request.messages.len(), 3);
    assert!(matches!(
        request.messages[2].content[0],
        Content::ToolResult { .. }
    ));
    let verify = advance(
        &model.checkpoint,
        Input::Model {
            response: ModelResponse::Final {
                output: json!("bad"),
            },
        },
    );
    let repair = advance(
        &verify.checkpoint,
        Input::Verification {
            passed: false,
            feedback: "use JSON".into(),
        },
    );
    assert!(matches!(repair.action, Action::CallModel { .. }));
    let verify = advance(
        &repair.checkpoint,
        Input::Model {
            response: ModelResponse::Final {
                output: json!({"answer":42}),
            },
        },
    );
    let done = advance(
        &verify.checkpoint,
        Input::Verification {
            passed: true,
            feedback: String::new(),
        },
    );
    assert_eq!(
        done.action,
        Action::Complete {
            output: json!({"answer":42})
        }
    );
    assert!(Engine::new(AgentLoop)
        .advance(
            &config(),
            &done.checkpoint,
            Input::Start {
                value: json!("replay")
            }
        )
        .is_err());
}
#[test]
fn rejects_missing_duplicate_and_unrelated_tool_results() {
    let model = advance(
        &Engine::new(AgentLoop).initial_state(),
        Input::Start {
            value: json!("task"),
        },
    );
    let tools = advance(
        &model.checkpoint,
        Input::Model {
            response: ModelResponse::ToolCalls {
                calls: vec![ToolCall {
                    provider_metadata: serde_json::Value::Null,
                    call_id: "a".into(),
                    name: "query".into(),
                    arguments: json!({}),
                }],
            },
        },
    );
    let result = |id: &str| ToolResult {
        call_id: id.into(),
        outcome: ToolOutcome::Success { value: json!(0) },
    };
    for results in [vec![], vec![result("a"), result("a")], vec![result("b")]] {
        assert!(Engine::new(AgentLoop)
            .advance(&config(), &tools.checkpoint, Input::Tools { results })
            .is_err());
    }
}

#[test]
fn optional_user_question_survives_checkpoint_and_correlates_reply() {
    let mut config = config();
    config.allow_user_input = true;
    let engine = Engine::new(AgentLoop);
    let start = engine
        .advance(
            &config,
            &engine.initial_state(),
            Input::Start {
                value: json!("plan a trip"),
            },
        )
        .unwrap();
    let Action::CallModel { request } = &start.action else {
        panic!()
    };
    assert_eq!(request.tools.last().unwrap().name, "ask_user");
    let call = ToolCall {
        call_id: "question-1".into(),
        name: "ask_user".into(),
        arguments: json!({"prompt":"Which city?"}),
        provider_metadata: json!({"signature":"preserved"}),
    };
    let waiting = engine
        .advance(
            &config,
            &start.checkpoint,
            Input::Model {
                response: ModelResponse::ToolCalls {
                    calls: vec![call.clone()],
                },
            },
        )
        .unwrap();
    assert_eq!(
        waiting.action,
        Action::WaitForInput {
            prompt: "Which city?".into()
        }
    );
    let restored =
        serde_json::from_value(serde_json::to_value(waiting.checkpoint).unwrap()).unwrap();
    let resumed = Engine::new(AgentLoop)
        .advance(
            &config,
            &restored,
            Input::User {
                value: json!("Boston"),
            },
        )
        .unwrap();
    let Action::CallModel { request } = resumed.action else {
        panic!()
    };
    assert_eq!(
        request.messages[1].content,
        vec![Content::ToolCall { call: call.clone() }]
    );
    assert_eq!(
        request.messages[2].content,
        vec![Content::ToolResult {
            result: ToolResult {
                call_id: "question-1".into(),
                outcome: ToolOutcome::Success {
                    value: json!("Boston")
                }
            }
        }]
    );
    let mut other = call.clone();
    other.call_id = "other".into();
    other.name = "write".into();
    assert!(engine
        .advance(
            &config,
            &start.checkpoint,
            Input::Model {
                response: ModelResponse::ToolCalls {
                    calls: vec![call.clone(), other]
                }
            }
        )
        .is_err());
    let mut invalid = call;
    invalid.arguments = json!({"prompt":" "});
    assert!(engine
        .advance(
            &config,
            &start.checkpoint,
            Input::Model {
                response: ModelResponse::ToolCalls {
                    calls: vec![invalid]
                }
            }
        )
        .is_err());
}
