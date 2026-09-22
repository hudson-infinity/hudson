use crate::{Config, HarnessError, Message, ModelRequest};

/// Preserve complete messages/exchanges. No silent truncation or invented summarization.
pub fn build(config: &Config, messages: Vec<Message>) -> Result<ModelRequest, HarnessError> {
    let request = ModelRequest {
        instructions: config.instructions.clone(),
        model: config.model.clone(),
        messages,
        tools: config.model_tools()?,
    };
    let bytes = serde_json::to_vec(&request).map_err(|e| HarnessError::Invalid(e.to_string()))?;
    if bytes.len() > config.max_context_bytes {
        return Err(HarnessError::ContextLimit);
    }
    Ok(request)
}
