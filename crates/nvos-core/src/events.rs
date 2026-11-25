//! Intra-tick communication message types.
//!
//! During each tick, subsystems communicate with the kernel engine via typed
//! messages rather than direct state mutation:
//!
//! - `KernelRequest` — a subsystem's intent emitted during Phase 1.
//! - `KernelMutation` — a kernel-approved state change applied in Phase 2.
//! - `TraceEvent` — a telemetry record committed to the ring buffer.
//! - `SpanEvent` — a per-process state-transition span for the Gantt chart.
//! - `EngineStatus` — coarse engine state for the `watch` channel.

use crate::process::{BlockReason, ExitReason, Pid};
use crate::tick::Tick;

// ---------------------------------------------------------------------------
// Phase 1 request types (subsystem → kernel)
// ---------------------------------------------------------------------------

/// A request emitted by a subsystem during Phase 1 (read-only collection).
/// The kernel arbitrates these requests and converts approved ones into
/// `KernelMutation` values applied in Phase 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelRequest {
    /// A device DMA transfer completed; unblock the waiting process.
    DeviceInterruptComplete { device_id: u32, pid: Pid },
    /// A page-swap operation completed; unblock the faulting process.
    PageSwapComplete { pid: Pid, frame_id: u32 },
    /// A timer interrupt fired for a sleeping process that should now wake.
    TimerExpired { pid: Pid },
}

// ---------------------------------------------------------------------------
// Phase 2 mutation types (kernel → subsystem)
// ---------------------------------------------------------------------------

/// A kernel-approved state mutation applied to a subsystem in Phase 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelMutation {
    /// Move a process from `New` to `Ready`.
    AdmitProcess { pid: Pid },
    /// Move a process from `Blocked` or `Sleeping` to `Ready`.
    WakeProcess { pid: Pid },
    /// Move a process from `Ready` to `Running` (scheduler dispatch).
    DispatchProcess { pid: Pid },
    /// Preempt the currently running process back to `Ready`.
    PreemptProcess { pid: Pid },
    /// Mark a process as `Blocked(reason)`.
    BlockProcess { pid: Pid, reason: BlockReason },
    /// Mark a process as `Terminated(reason)`.
    TerminateProcess { pid: Pid, reason: ExitReason },
    /// Advance the tape cursor for the running process by one step.
    AdvanceTapeCursor { pid: Pid },
    /// Increment `current_token_progress` for a multi-tick CPU burst.
    IncrementTokenProgress { pid: Pid },
}

// ---------------------------------------------------------------------------
// Telemetry types
// ---------------------------------------------------------------------------

/// A telemetry record describing a single noteworthy event within a tick.
/// Stored in the ring buffer and exported to Perfetto/NDJSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    /// The tick at which this event occurred.
    pub tick: Tick,
    /// The process involved, if any.
    pub pid: Option<Pid>,
    /// Human-readable event description.
    pub kind: TraceEventKind,
}

/// Classification of a [`TraceEvent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEventKind {
    StateTransition { from: String, to: String },
    SchedulerDecision { action: String },
    MemoryFault { virtual_addr: u32 },
    IoRequest { device_id: u32, sector_count: u32 },
    IoComplete { device_id: u32 },
    FaultInjected { error_code: u32 },
    KernelPanic { subsystem: &'static str, message: String },
}

/// A per-process execution span for the Gantt chart.
///
/// Emitted via `tokio::sync::broadcast` (not `watch`) so that every
/// intermediate tick transition is captured without silent drops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanEvent {
    /// The process this span belongs to.
    pub pid: Pid,
    /// State label for the span color.
    pub state_label: String,
    /// First tick of this contiguous state span.
    pub start_tick: Tick,
    /// Last tick of this span (inclusive). `None` if the span is still open.
    pub end_tick: Option<Tick>,
}

/// Coarse engine status broadcast on the `tokio::sync::watch` channel.
/// Used by TUI and Web to update the tick counter and play/pause indicator.
/// Does NOT carry per-process spans (those go on the `broadcast` channel).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineStatus {
    /// Whether the simulation is currently advancing ticks.
    pub is_running: bool,
    /// The most recently completed tick.
    pub current_tick: Tick,
    /// Total number of active (non-terminated) processes.
    pub active_process_count: usize,
}
