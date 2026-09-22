use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CHECKPOINT_SCHEMA: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint {
    pub schema_version: u32,
    pub backend: String,
    pub backend_version: u32,
    pub step: u64,
    pub payload: Value,
}

impl Checkpoint {
    pub fn new(backend: &str, backend_version: u32) -> Self {
        Self {
            schema_version: CHECKPOINT_SCHEMA,
            backend: backend.into(),
            backend_version,
            step: 0,
            payload: Value::Null,
        }
    }
}
