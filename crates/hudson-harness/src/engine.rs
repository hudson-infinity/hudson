use crate::state::CHECKPOINT_SCHEMA;
use crate::{Backend, Checkpoint, Config, HarnessError, Input, Transition};

pub struct Engine<B> {
    backend: B,
}

impl<B: Backend> Engine<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
    pub fn initial_state(&self) -> Checkpoint {
        Checkpoint::new(self.backend.name(), self.backend.version())
    }
    pub fn advance(
        &self,
        config: &Config,
        state: &Checkpoint,
        input: Input,
    ) -> Result<Transition, HarnessError> {
        self.validate(state)?;
        let transition = self.backend.advance(config, state, input)?;
        self.validate(&transition.checkpoint)?;
        if transition.checkpoint.step
            != state
                .step
                .checked_add(1)
                .ok_or_else(|| HarnessError::Invalid("step overflow".into()))?
        {
            return Err(HarnessError::Invalid(
                "checkpoint must advance one step".into(),
            ));
        }
        Ok(transition)
    }
    fn validate(&self, state: &Checkpoint) -> Result<(), HarnessError> {
        if state.schema_version != CHECKPOINT_SCHEMA
            || state.backend != self.backend.name()
            || state.backend_version != self.backend.version()
        {
            return Err(HarnessError::IncompatibleCheckpoint);
        }
        Ok(())
    }
}
