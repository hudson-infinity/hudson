//! Explicit delegation to a preconfigured child runtime. No implicit tool inheritance.
use crate::{
    adapters::{
        models::ModelExecutor,
        tools::{ExecutionError, ToolExecutor, ToolRegistry},
    },
    definitions::digest,
    models::*,
    runtime::Runtime,
    Result,
};
use hudson_harness::Backend;
use serde_json::json;

/// Register a child as a parent-visible tool. The application selects the child
/// actor, pinned Agent version, provider, and tools before exposing delegation.
/// Child execution is synchronous and bounded; waiting children return a status
/// and run ID rather than pretending their task is complete.
pub fn register<B, M, T>(
    registry: &mut ToolRegistry,
    child: Runtime<B, M, T>,
    actor: Actor,
    agent: VersionRef,
    name: &str,
    description: &str,
    policy: &str,
) -> Result<Tool>
where
    B: Backend + 'static,
    M: ModelExecutor + 'static,
    T: ToolExecutor + 'static,
{
    Ok(register_with_join(registry, child, actor, agent, name, description, policy)?.remove(0))
}

/// Return both delegation and join tool definitions. Publish both for parents
/// that need to continue a child which previously returned a waiting status.
pub fn register_with_join<B, M, T>(
    registry: &mut ToolRegistry,
    child: Runtime<B, M, T>,
    actor: Actor,
    agent: VersionRef,
    name: &str,
    description: &str,
    policy: &str,
) -> Result<Vec<Tool>>
where
    B: Backend + 'static,
    M: ModelExecutor + 'static,
    T: ToolExecutor + 'static,
{
    register_shared_with_join(
        registry,
        std::sync::Arc::new(std::sync::Mutex::new(child)),
        actor,
        agent,
        name,
        description,
        policy,
    )
}

/// Share the same child executor with an embedding host's resume/approval routes.
pub fn register_shared_with_join<B, M, T>(
    registry: &mut ToolRegistry,
    child: std::sync::Arc<std::sync::Mutex<Runtime<B, M, T>>>,
    actor: Actor,
    agent: VersionRef,
    name: &str,
    description: &str,
    policy: &str,
) -> Result<Vec<Tool>>
where
    B: Backend + 'static,
    M: ModelExecutor + 'static,
    T: ToolExecutor + 'static,
{
    register_shared(
        registry,
        child,
        actor,
        agent,
        name,
        description,
        policy,
        false,
    )
}

/// Register submission/inspection tools for a durable external scheduler.
/// These callbacks never execute a child's model or tools themselves.
#[allow(clippy::too_many_arguments)]
pub(crate) fn register_shared<B, M, T>(
    registry: &mut ToolRegistry,
    child: std::sync::Arc<std::sync::Mutex<Runtime<B, M, T>>>,
    actor: Actor,
    agent: VersionRef,
    name: &str,
    description: &str,
    policy: &str,
    deferred: bool,
) -> Result<Vec<Tool>>
where
    B: Backend + 'static,
    M: ModelExecutor + 'static,
    T: ToolExecutor + 'static,
{
    let key = format!(
        "hudson.delegate.{}",
        if deferred {
            digest(&(&actor, &agent, name, description, "temporal"))?
        } else {
            digest(&(&actor, &agent, name, description))?
        }
    );
    let (mut task_schema, default_text_task) = child
        .lock()
        .map_err(|_| crate::Error::Conflict("child runtime unavailable".into()))?
        .store
        .read(|data| {
            let child_agent = data
                .agents
                .get(&(actor.workspace_id.clone(), agent.clone()))
                .ok_or(crate::Error::NotFound)?;
            Ok((
                child_agent
                    .input_schema
                    .clone()
                    .unwrap_or_else(|| json!({"type":"string","minLength":1,"maxLength":16384})),
                child_agent.input_schema.is_none(),
            ))
        })?;
    if !default_text_task {
        // A nested schema needs its own resource boundary: #/$defs references
        // must keep resolving within the child's schema, not the tool wrapper.
        let resource_id = format!("urn:hudson:task:{}", digest(&task_schema)?);
        if let Some(object) = task_schema.as_object_mut() {
            object.entry("$id").or_insert_with(|| json!(resource_id));
        }
    }
    let tool = Tool {
        id: key.clone(),
        workspace_id: actor.workspace_id.clone(),
        version: 1,
        schema_version: SCHEMA_VERSION,
        name: name.into(),
        description: description.into(),
        input_schema: json!({"type":"object","properties":{"task":task_schema},"required":["task"],"additionalProperties":false}),
        output_schema: None,
        execution: Execution::Registered { key: key.clone() },
        credential_ref: None,
        policy_ref: policy.into(),
        // Delegation can cause child writes; the parent must explicitly permit it.
        effect: Effect::Write,
        created_at: 0,
    };
    let mut join_tool = tool.clone();
    let join_key = format!("{key}.join");
    join_tool.id = join_key.clone();
    join_tool.name = format!("join_{name}");
    join_tool.description =
        "Continue or inspect an existing child run from this parent. Do not submit the task again."
            .into();
    join_tool.execution = Execution::Registered {
        key: join_key.clone(),
    };
    join_tool.input_schema = json!({"type":"object","properties":{"run_id":{"type":"string","format":"uuid"}},"required":["run_id"],"additionalProperties":false});
    let delegated = child.clone();
    let delegate_actor = actor.clone();
    let delegate_agent = agent.clone();
    registry.register(key, move |invocation| {
        let mut child = delegated
            .lock()
            .map_err(|_| ExecutionError::Unknown("child runtime unavailable".into()))?;
        let actor = &delegate_actor;
        let task = invocation
            .arguments
            .get("task")
            .cloned()
            .ok_or_else(|| ExecutionError::Failed("subagent task is required".into()))?;
        if default_text_task && task.as_str().is_some_and(|text| text.trim().is_empty()) {
            return Err(ExecutionError::Failed("subagent task is empty".into()));
        }
        let request_key = format!("delegate:{}", invocation.operation_id);
        let id = child
            .submit(actor, delegate_agent.clone(), task, Some(request_key))
            .map_err(|_| {
                ExecutionError::Unknown(
                    "child submission failed; inspect persisted run before retry".into(),
                )
            })?;
        if child
            .store
            .link_child(actor, id, invocation.operation_id)
            .is_err()
        {
            let _ = child.cancel(actor, id);
            return Err(ExecutionError::Unknown(format!(
                "child run {id} could not attach to an active parent"
            )));
        }
        if deferred {
            inspect(&child, actor, id)
        } else {
            drive(&mut child, actor, id)
        }
    })?;
    registry.register(join_key, move |invocation| {
        let id = invocation.arguments["run_id"]
            .as_str()
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .ok_or_else(|| ExecutionError::Failed("valid child run ID required".into()))?;
        let mut child = child
            .lock()
            .map_err(|_| ExecutionError::Unknown("child runtime unavailable".into()))?;
        child
            .store
            .validate_child_access(&actor, id, invocation.operation_id, &agent)
            .map_err(|_| {
                ExecutionError::Failed("child does not belong to this parent and specialist".into())
            })?;
        if deferred {
            inspect(&child, &actor, id)
        } else {
            drive(&mut child, &actor, id)
        }
    })?;
    Ok(vec![tool, join_tool])
}

fn drive<B: Backend, M: ModelExecutor, T: ToolExecutor>(
    child: &mut Runtime<B, M, T>,
    actor: &Actor,
    id: uuid::Uuid,
) -> std::result::Result<serde_json::Value, ExecutionError> {
    for _ in 0..512 {
        let view = child
            .tick(actor, id)
            .map_err(|_| ExecutionError::Unknown(format!("child run {id} requires inspection")))?;
        if view.status.terminal()
            || matches!(view.status, RunStatus::Waiting | RunStatus::Cancelling)
        {
            return Ok(
                json!({"child_run":id,"status":view.status,"result":view.result,"wait":view.wait,"reason":view.reason,"usage":view.usage}),
            );
        }
        // Another executor may own an active effect. Return its handle instead
        // of polling it or dispatching a duplicate from this parent callback.
        let operations = child
            .store
            .operations(actor, id)
            .map_err(|_| ExecutionError::Unknown(format!("cannot inspect child run {id}")))?;
        if operations.iter().any(|o| {
            matches!(
                o.status,
                OperationStatus::Running | OperationStatus::Unknown
            )
        }) {
            return Ok(json!({"child_run":id,"status":view.status,"result":null}));
        }
    }
    Err(ExecutionError::Unknown(format!(
        "child run {id} reached delegation driver limit"
    )))
}

fn inspect<B: Backend, M: ModelExecutor, T: ToolExecutor>(
    child: &Runtime<B, M, T>,
    actor: &Actor,
    id: uuid::Uuid,
) -> std::result::Result<serde_json::Value, ExecutionError> {
    let view = child
        .store
        .inspect(actor, id)
        .map_err(|_| ExecutionError::Unknown(format!("cannot inspect child run {id}")))?;
    Ok(
        json!({"child_run":id,"status":view.status,"result":view.result,"wait":view.wait,"reason":view.reason,"usage":view.usage}),
    )
}
