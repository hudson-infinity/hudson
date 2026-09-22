use crate::models::Tool;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum ExecutionError {
    Failed(String),
    /// Dispatch may have taken effect. No automatic retry is permitted.
    Unknown(String),
}

/// Created only after runtime admission. Trusted executors must honor the fixed target.
pub struct Invocation<'a> {
    pub operation_id: Uuid,
    pub tool: &'a Tool,
    pub arguments: &'a Value,
}

pub trait ToolExecutor: Send {
    fn execute(&mut self, invocation: Invocation<'_>) -> Result<Value, ExecutionError>;
}

/// Trusted application functions, selected by the tool's immutable execution key.
/// Construct this registry at startup, then pass it to `Runtime::new`.
#[derive(Default)]
pub struct ToolRegistry {
    handlers: std::collections::BTreeMap<String, Box<Handler>>,
}

type Handler = dyn for<'a> FnMut(Invocation<'a>) -> Result<Value, ExecutionError> + Send;

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registration is explicit: duplicates never silently replace an existing function.
    pub fn register<F>(&mut self, key: impl Into<String>, handler: F) -> crate::Result<()>
    where
        F: for<'a> FnMut(Invocation<'a>) -> Result<Value, ExecutionError> + Send + 'static,
    {
        let key = key.into();
        if key.trim().is_empty() {
            return Err(crate::Error::Invalid("tool execution key is empty".into()));
        }
        match self.handlers.entry(key) {
            std::collections::btree_map::Entry::Occupied(_) => Err(crate::Error::Conflict(
                "tool execution key already registered".into(),
            )),
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Box::new(handler));
                Ok(())
            }
        }
    }
}

impl ToolExecutor for ToolRegistry {
    fn execute(&mut self, invocation: Invocation<'_>) -> Result<Value, ExecutionError> {
        let crate::models::Execution::Registered { key } = &invocation.tool.execution else {
            return Err(ExecutionError::Failed(
                "tool requires another executor".into(),
            ));
        };
        let handler = self
            .handlers
            .get_mut(key)
            .ok_or_else(|| ExecutionError::Failed("tool execution key is not registered".into()))?;
        // A panic can happen after a write. Preserve uncertainty rather than claiming
        // that the effect failed safely and could be replayed.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(invocation)))
            .unwrap_or_else(|_| Err(ExecutionError::Unknown("tool handler panicked".into())))
    }
}
