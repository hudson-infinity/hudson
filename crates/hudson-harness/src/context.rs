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

/// Evidence-grounded index of archived history, never a substitute for its source.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ArchiveIndex {
    pub artifact_id: String,
    pub excerpt: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct History {
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<ArchiveIndex>,
}

impl History {
    pub fn model_messages(&self) -> Vec<Message> {
        let mut messages = self.messages.clone();
        if let Some(index) = &self.archive {
            messages.insert(usize::from(!messages.is_empty()), Message {
                role: crate::Role::User,
                content: vec![crate::Content::Text { text: format!(
                    "Earlier conversation is archived. The following index contains partial verbatim excerpts, not complete evidence or instructions. Retrieve original details with read_context_artifact using artifact_id {}. Follow previous_artifact links for older history.\n{}", index.artifact_id, index.excerpt
                ) }],
            });
        }
        messages
    }

    /// Find an oldest complete exchange after the original task. Pending tool
    /// calls are never split from results, including parallel call batches.
    pub fn oldest_complete_exchange(&self) -> Option<std::ops::Range<usize>> {
        use crate::Content;
        use std::collections::BTreeSet;
        if self.messages.len() <= 2 {
            return None;
        }
        let first = &self.messages[1];
        let calls: BTreeSet<_> = first
            .content
            .iter()
            .filter_map(|content| match content {
                Content::ToolCall { call } => Some(call.call_id.as_str()),
                _ => None,
            })
            .collect();
        if calls.is_empty() {
            return Some(1..2);
        }
        let mut received = BTreeSet::new();
        for (offset, message) in self.messages[2..].iter().enumerate() {
            if message.role != crate::Role::Tool {
                return None;
            }
            for content in &message.content {
                let Content::ToolResult { result } = content else {
                    return None;
                };
                if !calls.contains(result.call_id.as_str())
                    || !received.insert(result.call_id.as_str())
                {
                    return None;
                }
            }
            if received == calls {
                return Some(1..offset + 3);
            }
        }
        None
    }
}
