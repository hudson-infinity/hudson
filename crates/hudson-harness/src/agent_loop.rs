//! Resumable model → tools → model loop, independent of providers and storage.
use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct AgentLoop;

#[derive(Default, Serialize, Deserialize)]
struct State {
    messages: Vec<Message>,
    phase: Phase,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Phase {
    #[default]
    Start,
    Model,
    Tools {
        calls: Vec<ToolCall>,
    },
    UserInput {
        call_id: String,
    },
    Verification {
        candidate: Value,
    },
    Done,
}

fn invalid(message: &str) -> HarnessError {
    HarnessError::Invalid(message.into())
}

impl Backend for AgentLoop {
    fn name(&self) -> &str {
        "hudson"
    }
    fn version(&self) -> u32 {
        1
    }

    fn advance(
        &self,
        config: &Config,
        checkpoint: &Checkpoint,
        input: Input,
    ) -> Result<Transition, HarnessError> {
        let mut state: State = if checkpoint.step == 0 && checkpoint.payload.is_null() {
            State::default()
        } else {
            serde_json::from_value(checkpoint.payload.clone())
                .map_err(|e| invalid(&e.to_string()))?
        };
        let action = match (&state.phase, input) {
            (Phase::Start, Input::Start { value }) => {
                state.messages.push(Message {
                    role: Role::User,
                    content: vec![Content::Json { value }],
                });
                request(config, &mut state)?
            }
            (
                Phase::Model,
                Input::Model {
                    response: ModelResponse::ToolCalls { calls },
                },
            ) => {
                let ids: BTreeSet<_> = calls.iter().map(|c| c.call_id.as_str()).collect();
                if calls.is_empty() || ids.len() != calls.len() || ids.contains("") {
                    return Err(invalid("tool calls require unique nonempty IDs"));
                }
                state.messages.push(Message {
                    role: Role::Assistant,
                    content: calls
                        .iter()
                        .cloned()
                        .map(|call| Content::ToolCall { call })
                        .collect(),
                });
                if config.allow_user_input && calls.iter().any(|call| call.name == "ask_user") {
                    if calls.len() != 1 {
                        return Err(invalid("ask_user must be requested alone"));
                    }
                    let call = &calls[0];
                    let args = call
                        .arguments
                        .as_object()
                        .ok_or_else(|| invalid("ask_user arguments must be an object"))?;
                    let prompt = args
                        .get("prompt")
                        .and_then(Value::as_str)
                        .filter(|prompt| !prompt.trim().is_empty() && prompt.len() <= 16384)
                        .ok_or_else(|| {
                            invalid("ask_user requires a nonempty prompt of at most 16 KiB")
                        })?;
                    if args.len() != 1 {
                        return Err(invalid("unexpected ask_user argument"));
                    }
                    state.phase = Phase::UserInput {
                        call_id: call.call_id.clone(),
                    };
                    Action::WaitForInput {
                        prompt: prompt.into(),
                    }
                } else {
                    state.phase = Phase::Tools {
                        calls: calls.clone(),
                    };
                    Action::ExecuteTools { calls }
                }
            }
            (Phase::UserInput { call_id }, Input::User { value }) => {
                state.messages.push(Message {
                    role: Role::Tool,
                    content: vec![Content::ToolResult {
                        result: ToolResult {
                            call_id: call_id.clone(),
                            outcome: ToolOutcome::Success { value },
                        },
                    }],
                });
                request(config, &mut state)?
            }
            (Phase::Tools { calls }, Input::Tools { results }) => {
                let expected: BTreeSet<_> = calls.iter().map(|c| &c.call_id).collect();
                let received: BTreeSet<_> = results.iter().map(|r| &r.call_id).collect();
                if expected != received || received.len() != results.len() {
                    return Err(invalid("tool results must match the pending batch exactly"));
                }
                // Keep the original call order even if tools finish out of order.
                for call in calls {
                    let result = results
                        .iter()
                        .find(|r| r.call_id == call.call_id)
                        .unwrap()
                        .clone();
                    state.messages.push(Message {
                        role: Role::Tool,
                        content: vec![Content::ToolResult { result }],
                    });
                }
                request(config, &mut state)?
            }
            (
                Phase::Model,
                Input::Model {
                    response: ModelResponse::Final { output },
                },
            ) => {
                state.messages.push(Message {
                    role: Role::Assistant,
                    content: vec![Content::Json {
                        value: output.clone(),
                    }],
                });
                state.phase = Phase::Verification {
                    candidate: output.clone(),
                };
                Action::Verify { candidate: output }
            }
            (Phase::Verification { candidate }, Input::Verification { passed: true, .. }) => {
                let output = candidate.clone();
                state.phase = Phase::Done;
                Action::Complete { output }
            }
            (
                Phase::Verification { .. },
                Input::Verification {
                    passed: false,
                    feedback,
                },
            ) => {
                state.messages.push(Message { role: Role::User, content: vec![Content::Text { text: format!("The output did not pass verification. Correct it using this feedback: {feedback}") }] });
                request(config, &mut state)?
            }
            _ => return Err(invalid("input does not match the pending loop action")),
        };
        Ok(Transition {
            checkpoint: Checkpoint {
                step: checkpoint
                    .step
                    .checked_add(1)
                    .ok_or_else(|| invalid("step overflow"))?,
                payload: serde_json::to_value(state).map_err(|e| invalid(&e.to_string()))?,
                ..checkpoint.clone()
            },
            action,
        })
    }
}

fn request(config: &Config, state: &mut State) -> Result<Action, HarnessError> {
    let request = context::build(config, state.messages.clone())?;
    state.phase = Phase::Model;
    Ok(Action::CallModel { request })
}
