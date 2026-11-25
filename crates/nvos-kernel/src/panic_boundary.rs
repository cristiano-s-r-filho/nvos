//! Safe panic isolation boundary ("Virtual BSoD").
//!
//! Subsystem calls (such as pluggable student schedulers or memory policies)
//! are wrapped in `catch_unwind`. If a panic occurs, it is captured,
//! formatted into an engine error, and safely reported without aborting the host process.

use std::panic::{catch_unwind, AssertUnwindSafe};
use nvos_core::error::{EngineErrorKind, KernelEngineError};

/// Subsystem panic wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedKernelPanic {
    /// Name of the subsystem that panicked.
    pub subsystem: &'static str,
    /// Extracted panic message payload.
    pub reason: String,
}

impl std::fmt::Display for SimulatedKernelPanic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SimulatedKernelPanic in subsystem '{}': {}",
            self.subsystem, self.reason
        )
    }
}

impl std::error::Error for SimulatedKernelPanic {}

/// Safely execute a fallible subsystem closure, isolating any panics.
///
/// Converts host thread panics into [`KernelEngineError`].
pub fn safe_subsystem_dispatch<F, R>(
    subsystem_name: &'static str,
    action: F,
) -> Result<R, KernelEngineError>
where
    F: FnOnce() -> Result<R, KernelEngineError>,
{
    match catch_unwind(AssertUnwindSafe(action)) {
        Ok(result) => result,
        Err(panic_payload) => {
            let reason = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic payload".to_string()
            };

            Err(KernelEngineError {
                subsystem: subsystem_name,
                kind: EngineErrorKind::Panic { message: reason },
            })
        }
    }
}
