use crate::{Checkpoint, Config, Input, Transition};

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("unsupported checkpoint version or backend")]
    IncompatibleCheckpoint,
    #[error("context exceeds configured byte limit")]
    ContextLimit,
    #[error("invalid harness transition: {0}")]
    Invalid(String),
}

/// Implement with a pinned loop-library adapter. All effects are returned as actions.
pub trait Backend: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> u32;
    fn advance(
        &self,
        config: &Config,
        state: &Checkpoint,
        input: Input,
    ) -> Result<Transition, HarnessError>;
}
