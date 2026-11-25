//! `nvos-cpu` — pluggable CPU scheduling policies for `nvos`.
//!
//! Schedulers operate strictly on `Pid` and `PriorityLevel` abstractions
//! without inspecting memory tables or file descriptors. All methods return `Result`
//! to ensure fault isolation.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::all)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

use std::fmt;
use nvos_core::process::{BlockReason, Pid, PriorityLevel};
use nvos_core::tick::Tick;

pub mod fcfs;
pub mod mlfq;
pub mod round_robin;
pub mod sjf;

pub use fcfs::FcfsScheduler;
pub use mlfq::MlfqScheduler;
pub use round_robin::RoundRobinScheduler;
pub use sjf::SjfScheduler;

/// Scheduling decision returned by [`Scheduler::tick`].
///
/// Preempt carries no payload; the kernel decides who to schedule next
/// via [`Scheduler::schedule_next`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingAction {
    /// Keep current process running on the CPU core.
    Continue,
    /// Current process voluntarily yields the CPU.
    Yield,
    /// Current process should be preempted (e.g. quantum exhausted or higher priority arrived).
    Preempt,
}

/// Errors produced by scheduler implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerError {
    /// Attempted to add a process that is already tracked in the ready queues.
    DuplicateProcess(Pid),
    /// Process was not found in active ready/wait queues.
    ProcessNotFound(Pid),
    /// Internal queue invariant violation or corruption.
    InternalCorruption(String),
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateProcess(pid) => write!(f, "process {pid:?} already enrolled in scheduler"),
            Self::ProcessNotFound(pid) => write!(f, "process {pid:?} not found in ready queues"),
            Self::InternalCorruption(msg) => write!(f, "internal scheduler queue corruption: {msg}"),
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Pluggable CPU scheduler trait.
pub trait Scheduler: Send + Sync {
    /// Name of the scheduling policy (e.g., "FCFS", "RoundRobin-Q4", "MLFQ").
    fn name(&self) -> &'static str;

    /// Register a newly arrived or awakened process into the ready queue.
    fn add_process(&mut self, pid: Pid, priority: PriorityLevel) -> Result<(), SchedulerError>;

    /// Select the next process to execute on the CPU core.
    /// Called once per tick in Phase 2 Step 5.
    fn schedule_next(&mut self, current_tick: Tick) -> Result<Option<Pid>, SchedulerError>;

    /// Advance quantum tracking for the active running process.
    /// Returns the scheduling action for the current tick.
    fn tick(&mut self, running_pid: Option<Pid>) -> Result<SchedulingAction, SchedulerError>;

    /// Notify scheduler that a process has blocked on I/O, sleep, or page fault.
    fn on_process_blocked(&mut self, pid: Pid, reason: BlockReason) -> Result<(), SchedulerError>;

    /// Notify scheduler that a process has terminated.
    fn on_process_terminated(&mut self, pid: Pid) -> Result<(), SchedulerError>;
}
