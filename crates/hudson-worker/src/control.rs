//! Local management commands never construct a provider or execute a tool.
use crate::config::Args;
use hudson_core::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, ToolRegistry},
    },
    models::*,
    runtime::Runtime,
    storage::Store,
};
use hudson_harness::{AgentLoop, ModelRequest, ModelResponse};
struct NoModel;
impl ModelExecutor for NoModel {
    fn call(&mut self, _: &ModelRequest) -> Result<ModelResponse, ExecutionError> {
        Err(ExecutionError::Failed(
            "management commands cannot call models".into(),
        ))
    }
}
pub fn execute(args: &Args) -> Result<bool, Box<dyn std::error::Error>> {
    if args.inspect_run.is_none()
        && args.cancel_run.is_none()
        && args.reply_run.is_none()
        && args.inspect_operation.is_none()
        && args.approve_operation.is_none()
        && args.deny_operation.is_none()
        && args.mark_interrupted.is_none()
        && args.abandon_model.is_none()
        && args.reconcile_operation.is_none()
    {
        return Ok(false);
    }
    let store = Store::postgres_local(
        "/tmp",
        args.database
            .as_deref()
            .ok_or("management requires --database")?,
        &args.namespace,
    )?;
    let actor = Actor {
        workspace_id: args.workspace_id.clone(),
        id: args.actor_id.clone(),
    };
    let runtime = Runtime::new(store, AgentLoop, NoModel, ToolRegistry::new());
    let output = if let Some(id) = args.inspect_run {
        let operations = runtime
            .store
            .operations(&actor, id)?
            .iter()
            .map(OperationView::from)
            .collect::<Vec<_>>();
        serde_json::json!({"run":runtime.store.inspect(&actor,id)?,"operations":operations,"events":runtime.store.events(&actor,id,0)?,"children":runtime.store.children(&actor,id)?})
    } else if let Some(id) = args.reply_run {
        let value = if let Some(text) = &args.reply_text {
            serde_json::Value::String(text.clone())
        } else {
            read_result(args.reply_file.as_deref().ok_or("reply file is required")?)?
        };
        runtime.provide_input(
            &actor,
            id,
            args.question_id.ok_or("question ID is required")?,
            args.request_key
                .as_deref()
                .ok_or("reply request key is required")?,
            value,
        )?;
        serde_json::to_value(runtime.store.inspect(&actor, id)?)?
    } else if let Some(id) = args.cancel_run {
        runtime.cancel(&actor, id)?;
        serde_json::to_value(runtime.store.inspect(&actor, id)?)?
    } else if let Some(id) = args.inspect_operation {
        let view = runtime.store.inspect_operation(&actor, id)?;
        let operation = runtime
            .store
            .operations(&actor, view.run_id)?
            .into_iter()
            .find(|operation| operation.meta.id == id)
            .ok_or("operation disappeared")?;
        let mut output = serde_json::to_value(view)?;
        // Local operators need the attempt identity to fence exactly what they observed.
        // Executable requests and credentials remain excluded.
        output["attempts"] = serde_json::to_value(operation.attempts)?;
        output
    } else if let Some(id) = args.mark_interrupted {
        runtime.mark_interrupted(
            &actor,
            id,
            args.attempt_id.ok_or("attempt ID is required")?,
            args.evidence
                .as_deref()
                .ok_or("executor-stop evidence is required")?,
        )?;
        serde_json::to_value(runtime.store.inspect_operation(&actor, id)?)?
    } else if let Some(id) = args.abandon_model {
        runtime.abandon_model_response(
            &actor,
            id,
            args.evidence.as_deref().ok_or("evidence is required")?,
        )?;
        serde_json::to_value(runtime.store.inspect_operation(&actor, id)?)?
    } else if let Some(id) = args.reconcile_operation {
        let value = read_result(
            args.result_file
                .as_deref()
                .ok_or("result file is required")?,
        )?;
        runtime.record_reconciled_tool_result(
            &actor,
            id,
            value,
            args.receipt
                .as_deref()
                .ok_or("destination receipt is required")?,
        )?;
        serde_json::to_value(runtime.store.inspect_operation(&actor, id)?)?
    } else {
        let (id, approved) = args
            .approve_operation
            .map(|id| (id, true))
            .or_else(|| args.deny_operation.map(|id| (id, false)))
            .ok_or("missing operation")?;
        let at = now();
        runtime.approve(&actor, id, approved, at, at + 300_000)?;
        serde_json::to_value(runtime.store.inspect_operation(&actor, id)?)?
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(true)
}

fn read_result(path: &std::path::Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    use std::io::Read;
    const MAX_BYTES: u64 = 1024 * 1024;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("result file exceeds 1 MiB".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
