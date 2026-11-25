//! `nvos-kernel` — deterministic discrete-event kernel core for `nvos`.
//!
//! Provides the process arena, Two-Phase Commit tick evaluation loop,
//! safe panic boundary, and async `SimulationRunner`.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::all)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::too_many_lines,
    clippy::collapsible_if,
    clippy::doc_markdown
)]

pub mod arena;
pub mod engine;
pub mod memory_bridge;
pub mod panic_boundary;
pub mod runner;

pub use arena::{ProcessArena, ProcessKey};
pub use engine::KernelEngine;
pub use memory_bridge::MemoryBridge;
pub use panic_boundary::{safe_subsystem_dispatch, SimulatedKernelPanic};
pub use runner::{EngineCommand, SimulationRunner};
