//! Run: cargo run -p hudson-core --example agent -- 'Use sum to add 17 and 25'
use hudson_core::{
    adapters::{
        chat::ChatModel,
        tools::{ExecutionError, ToolRegistry},
    },
    models::*,
    runtime::Runtime,
    security::Policy,
    storage::MemoryStore,
};
use hudson_harness::AgentLoop;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let task = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if task.trim().is_empty() {
        return Err("usage: agent <task> (requires OPENAI_API_KEY)".into());
    }
    let endpoint = std::env::var("HUDSON_MODEL_ENDPOINT")
        .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".into());
    let key = std::env::var("OPENAI_API_KEY").ok();
    if endpoint == "https://api.openai.com/v1/chat/completions"
        && key.as_deref().is_none_or(str::is_empty)
    {
        return Err("set OPENAI_API_KEY before running".into());
    }
    let model_name = std::env::var("HUDSON_MODEL").unwrap_or_else(|_| "gpt-4.1-mini".into());
    let model = ChatModel::new(&endpoint, key, 256)?;
    let store = MemoryStore::default();
    let actor = Actor {
        workspace_id: "local".into(),
        id: "developer".into(),
    };
    let tool = Tool {
        id: "sum".into(),
        workspace_id: actor.workspace_id.clone(),
        version: 1,
        schema_version: SCHEMA_VERSION,
        name: "sum".into(),
        description: "Add two finite numbers accurately.".into(),
        input_schema: json!({"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}},"required":["a","b"],"additionalProperties":false}),
        output_schema: None,
        execution: Execution::Registered { key: "sum".into() },
        credential_ref: None,
        policy_ref: "local-tools".into(),
        effect: Effect::Read,
        created_at: now(),
    };
    let agent = Agent {
        allow_user_input: false,
        id:"assistant".into(), workspace_id:actor.workspace_id.clone(), version:1, schema_version:SCHEMA_VERSION,
        name:"Hudson assistant".into(), instructions:"Help with the user's task. Use available tools when useful. Give a concise final answer.".into(),
        model:model_name, tools:vec![AgentTool { tool_ref:tool.reference(), alias:tool.name.clone() }],
        input_schema:None, output_schema:None,
        limits:Limits { max_model_calls:3, max_harness_steps:24, max_operations:8, ..Limits::default() }, created_at:now(),
    };
    let reference = agent.reference();
    store.set_policy(
        &actor.workspace_id,
        "local-tools",
        Policy {
            actors: [actor.id.clone()].into(),
            approvers: Default::default(),
            require_approval: false,
        },
    )?;
    store.publish_tool(tool)?;
    store.publish_agent(agent)?;
    let mut tools = ToolRegistry::new();
    tools.register("sum", |call| {
        let a = call.arguments["a"]
            .as_f64()
            .ok_or_else(|| ExecutionError::Failed("a must be a number".into()))?;
        let b = call.arguments["b"]
            .as_f64()
            .ok_or_else(|| ExecutionError::Failed("b must be a number".into()))?;
        let sum = a + b;
        if !sum.is_finite() {
            return Err(ExecutionError::Failed(
                "sum exceeds finite number range".into(),
            ));
        }
        Ok(json!({"sum":sum}))
    })?;
    let mut runtime = Runtime::new(store, AgentLoop, model, tools);
    let id = runtime.submit(&actor, reference, json!(task), None)?;
    for _ in 0..64 {
        let run = runtime.tick(&actor, id)?;
        if run.status.terminal() || matches!(run.status, RunStatus::Waiting | RunStatus::Cancelling)
        {
            println!("{}", serde_json::to_string_pretty(&run)?);
            if run.status != RunStatus::Completed {
                return Err("run did not complete; see status above".into());
            }
            return Ok(());
        }
    }
    Err("driver step limit reached".into())
}
