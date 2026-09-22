use crate::{models::Agent, storage::memory::Data, Error, Result};
use hudson_harness::{Config, ToolDescriptor};

pub(crate) fn project(agent: &Agent, data: &Data) -> Result<Config> {
    let tools = agent
        .tools
        .iter()
        .map(|binding| {
            let tool = data
                .tools
                .get(&(agent.workspace_id.clone(), binding.tool_ref.clone()))
                .ok_or(Error::NotFound)?;
            Ok(ToolDescriptor {
                name: binding.alias.clone(),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Config {
        instructions: agent.instructions.clone(),
        allow_user_input: agent.allow_user_input,
        model: agent.model.clone(),
        tools,
        max_context_bytes: agent.limits.max_context_bytes,
    })
}
