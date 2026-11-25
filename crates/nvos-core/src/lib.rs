//! `nvos-core` — foundational domain types for the nvos OS simulator.
//!
//! Zero external runtime dependencies (serde and slotmap are optional features).
//! All types in this crate must be usable without instantiating a kernel.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::all)]
#![allow(clippy::module_name_repetitions)]

pub mod error;
pub mod events;
pub mod process;
pub mod tape;
pub mod tick;

pub use error::{GuestTrap, KernelEngineError, SimulationError, TrapReason};
pub use events::{EngineStatus, KernelMutation, KernelRequest, SpanEvent, TraceEvent, TraceEventKind};
pub use process::{
    BlockReason, ExitReason, FileDescriptor, HeapBlockId, InodeId, MemoryLimits, PageTableId,
    Pid, PPid, PriorityLevel, ProcessControlBlock, ProcessState,
};
pub use tape::{FaultTarget, InstructionToken};
pub use tick::{DurationTicks, Tick};
