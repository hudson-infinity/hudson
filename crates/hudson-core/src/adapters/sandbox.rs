//! The first slice must fail closed; it never falls back to a host process.
use crate::{models::Execution, Error, Result};

pub fn ensure_supported(execution: &Execution) -> Result<()> {
    match execution {
        Execution::Registered { .. } => Ok(()),
        Execution::Http { .. } => Ok(()),
        Execution::Sandbox { .. } => Err(Error::Unsupported(
            "Hudson Sandbox is not integrated".into(),
        )),
    }
}
